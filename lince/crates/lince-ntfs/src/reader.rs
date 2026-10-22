use std::io;
use std::ops::Range;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::{
    apply_fixups, parse_attr_list, parse_boot_sector, parse_record, AttrList, BootSector,
    NonResidentAttr, NtfsError, ParsedRecord, ATTR_BITMAP, ATTR_DATA,
};

/// Fuente de bytes con lectura posicional (volumen o imagen de disco).
///
/// Debe permitir lecturas concurrentes desde varios hilos.
pub trait ReadAt: Sync {
    /// Llena `buf` completo con los bytes que empiezan en `offset`.
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()>;
}

/// Tramo contiguo de la MFT en el disco.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Extent {
    /// Desplazamiento dentro de la MFT (bytes).
    pub mft_offset: u64,
    /// Desplazamiento en el volumen (bytes).
    pub disk_offset: u64,
    pub len: u64,
}

/// Dónde está la MFT y qué registros están en uso.
#[derive(Debug, Clone)]
pub struct MftLayout {
    pub boot: BootSector,
    pub extents: Vec<Extent>,
    pub record_count: u64,
    /// Bit `i` = registro `i` en uso. Puede ser más corto que `record_count`.
    pub bitmap: Vec<u8>,
}

/// Concurrencia de la lectura.
#[derive(Debug, Clone, Copy)]
pub struct ReadOptions {
    /// Lecturas simultáneas (y hilos de parseo).
    pub threads: usize,
    /// Tamaño de cada lectura.
    pub chunk_bytes: usize,
}

/// Resultado de leer toda la MFT.
#[derive(Debug, Clone)]
pub struct MftScan {
    /// Registros en uso, ordenados por FRS.
    pub records: Vec<ParsedRecord>,
    /// Registros descartados por estar corruptos.
    pub bad_records: u64,
    pub bytes_read: u64,
}

impl ReadAt for [u8] {
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        let src = usize::try_from(offset)
            .ok()
            .and_then(|start| self.get(start..start.checked_add(buf.len())?))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::UnexpectedEof, "lectura fuera de la imagen")
            })?;
        buf.copy_from_slice(src);
        Ok(())
    }
}

impl ReadAt for std::fs::File {
    #[cfg(unix)]
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        std::os::unix::fs::FileExt::read_exact_at(self, buf, offset)
    }

    #[cfg(windows)]
    fn read_at(&self, offset: u64, buf: &mut [u8]) -> io::Result<()> {
        use std::os::windows::fs::FileExt;
        let mut done = 0;
        while done < buf.len() {
            match self.seek_read(&mut buf[done..], offset + done as u64)? {
                0 => return Err(io::ErrorKind::UnexpectedEof.into()),
                n => done += n,
            }
        }
        Ok(())
    }
}

/// Huecos de registros libres de hasta este tamaño se leen igual: una lectura
/// grande cuesta menos que dos pequeñas.
const MERGE_GAP: u64 = 64;

/// Localiza la MFT a partir del sector de arranque y su propio registro (0).
pub fn read_layout<R: ReadAt + ?Sized>(vol: &R) -> Result<MftLayout, NtfsError> {
    let mut boot_buf = vec![0u8; 4096];
    vol.read_at(0, &mut boot_buf)?;
    let boot = parse_boot_sector(&boot_buf)?;
    let rs = u64::from(boot.record_size);

    let mut rec0 = vec![0u8; boot.record_size as usize];
    vol.read_at(boot.mft_lcn * u64::from(boot.bytes_per_cluster), &mut rec0)?;
    apply_fixups(&mut rec0)?;
    let base =
        parse_record(0, &rec0, true)?.ok_or(NtfsError::BadRecord("$MFT marcado como libre"))?;
    let mut segments = base.nonresident;

    // MFT muy fragmentada: parte de sus tramos están en registros de extensión.
    if let Some(list) = base.attr_list {
        let entries = match list {
            AttrList::Resident(entries) => entries,
            AttrList::NonResident { data_size, runs } => {
                let seg = NonResidentAttr {
                    type_code: 0,
                    named: false,
                    lowest_vcn: 0,
                    data_size,
                    runs,
                };
                parse_attr_list(&read_segments(vol, &boot, &[&seg])?)?
            }
        };
        let mut ext: Vec<u64> = entries
            .iter()
            .filter(|e| !e.named && e.record_frs != 0)
            .filter(|e| e.type_code == ATTR_DATA || e.type_code == ATTR_BITMAP)
            .map(|e| e.record_frs)
            .collect();
        ext.sort_unstable();
        ext.dedup();
        for frs in ext {
            let known = extents_from(&boot, &unnamed(&segments, ATTR_DATA))?;
            let mut rec = vec![0u8; boot.record_size as usize];
            read_mft(vol, &known, rs, frs * rs, &mut rec)?;
            apply_fixups(&mut rec)?;
            let parsed = parse_record(frs, &rec, true)?
                .ok_or(NtfsError::BadRecord("registro de extensión de $MFT libre"))?;
            segments.extend(parsed.nonresident);
        }
    }

    let data = unnamed(&segments, ATTR_DATA);
    let first = data
        .first()
        .filter(|s| s.lowest_vcn == 0)
        .ok_or(NtfsError::BadRecord("$MFT sin $DATA"))?;
    let record_count = first.data_size / rs;
    let extents = extents_from(&boot, &data)?;

    // Sin $BITMAP legible se leen todos los registros (más lento, igual de correcto).
    let bitmap_segs = unnamed(&segments, ATTR_BITMAP);
    let bitmap = match bitmap_segs.first() {
        Some(s) if s.lowest_vcn == 0 => read_segments(vol, &boot, &bitmap_segs)?,
        _ => Vec::new(),
    };

    Ok(MftLayout {
        boot,
        extents,
        record_count,
        bitmap,
    })
}

/// Lee todos los registros en uso con `opts.threads` lecturas simultáneas.
/// Cada hilo parsea lo que lee, así que el parseo también va en paralelo.
pub fn read_records<R: ReadAt + ?Sized>(
    vol: &R,
    layout: &MftLayout,
    opts: &ReadOptions,
) -> Result<MftScan, NtfsError> {
    let rs = u64::from(layout.boot.record_size);
    let per_chunk = (opts.chunk_bytes as u64 / rs).max(1);
    let chunks = plan_chunks(&layout.bitmap, layout.record_count, per_chunk, MERGE_GAP);
    let threads = opts.threads.clamp(1, chunks.len().max(1));

    let next = AtomicUsize::new(0);
    let failed = AtomicBool::new(false);
    let first_err: Mutex<Option<NtfsError>> = Mutex::new(None);
    let done: Mutex<Vec<(usize, Vec<ParsedRecord>, u64)>> = Mutex::new(Vec::new());

    std::thread::scope(|s| {
        for _ in 0..threads {
            s.spawn(|| {
                let mut buf = vec![0u8; (per_chunk * rs) as usize];
                while !failed.load(Ordering::Relaxed) {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    let Some(range) = chunks.get(i) else { break };
                    let buf = &mut buf[..((range.end - range.start) * rs) as usize];
                    if let Err(e) = read_mft(vol, &layout.extents, rs, range.start * rs, buf) {
                        failed.store(true, Ordering::Relaxed);
                        first_err.lock().unwrap().get_or_insert(e);
                        break;
                    }
                    let (records, bad) = parse_chunk(range.start, buf, rs as usize, &layout.bitmap);
                    done.lock().unwrap().push((i, records, bad));
                }
            });
        }
    });

    if let Some(e) = first_err.into_inner().unwrap() {
        return Err(e);
    }
    let mut done = done.into_inner().unwrap();
    done.sort_unstable_by_key(|(i, _, _)| *i);
    let bytes_read = chunks.iter().map(|r| (r.end - r.start) * rs).sum();
    let bad_records = done.iter().map(|(_, _, bad)| bad).sum();
    let records = done.into_iter().flat_map(|(_, recs, _)| recs).collect();
    Ok(MftScan {
        records,
        bad_records,
        bytes_read,
    })
}

fn is_used(bitmap: &[u8], i: u64) -> bool {
    bitmap
        .get((i / 8) as usize)
        .is_none_or(|b| b & (1 << (i % 8)) != 0)
}

fn parse_chunk(
    first_frs: u64,
    buf: &mut [u8],
    rs: usize,
    bitmap: &[u8],
) -> (Vec<ParsedRecord>, u64) {
    let mut records = Vec::new();
    let mut bad = 0;
    for (k, rec) in buf.chunks_exact_mut(rs).enumerate() {
        let frs = first_frs + k as u64;
        if !is_used(bitmap, frs) || rec[..4] == [0, 0, 0, 0] {
            continue;
        }
        if &rec[..4] != b"FILE" || apply_fixups(rec).is_err() {
            bad += 1;
            continue;
        }
        match parse_record(frs, rec, false) {
            Ok(Some(r)) => records.push(r),
            Ok(None) => {}
            Err(_) => bad += 1,
        }
    }
    (records, bad)
}

/// Rangos de registros a leer: los que están en uso, uniendo huecos pequeños y
/// partidos en trozos de `per_chunk` registros como máximo.
fn plan_chunks(
    bitmap: &[u8],
    record_count: u64,
    per_chunk: u64,
    merge_gap: u64,
) -> Vec<Range<u64>> {
    let mut used: Vec<Range<u64>> = Vec::new();
    let mut i = 0;
    while i < record_count {
        let byte = (i / 8) as usize;
        if i % 8 == 0 && bitmap.get(byte) == Some(&0) {
            i += 8;
            continue;
        }
        if !is_used(bitmap, i) {
            i += 1;
            continue;
        }
        let start = i;
        while i < record_count && is_used(bitmap, i) {
            i += 1;
        }
        match used.last_mut() {
            Some(prev) if start - prev.end <= merge_gap => prev.end = i,
            _ => used.push(start..i),
        }
    }
    let mut chunks = Vec::new();
    for r in used {
        let mut s = r.start;
        while s < r.end {
            let e = (s + per_chunk).min(r.end);
            chunks.push(s..e);
            s = e;
        }
    }
    chunks
}

/// Lee `buf.len()` bytes de la MFT desde `mft_offset`, siguiendo sus tramos.
fn read_mft<R: ReadAt + ?Sized>(
    vol: &R,
    extents: &[Extent],
    record_size: u64,
    mft_offset: u64,
    buf: &mut [u8],
) -> Result<(), NtfsError> {
    let mut done = 0usize;
    while done < buf.len() {
        let off = mft_offset + done as u64;
        let idx = extents.partition_point(|e| e.mft_offset + e.len <= off);
        let ext = extents
            .get(idx)
            .filter(|e| e.mft_offset <= off)
            .ok_or(NtfsError::MftUnmapped(off / record_size))?;
        let within = off - ext.mft_offset;
        let n = (buf.len() - done).min((ext.len - within) as usize);
        vol.read_at(ext.disk_offset + within, &mut buf[done..done + n])?;
        done += n;
    }
    Ok(())
}

/// Segmentos sin nombre de un tipo de atributo, en orden de VCN.
fn unnamed(segments: &[NonResidentAttr], type_code: u32) -> Vec<&NonResidentAttr> {
    let mut v: Vec<_> = segments
        .iter()
        .filter(|s| s.type_code == type_code && !s.named)
        .collect();
    v.sort_by_key(|s| s.lowest_vcn);
    v
}

/// Convierte los tramos del `$DATA` de la `$MFT` en tramos de bytes.
fn extents_from(
    boot: &BootSector,
    segments: &[&NonResidentAttr],
) -> Result<Vec<Extent>, NtfsError> {
    let bpc = u64::from(boot.bytes_per_cluster);
    let mut extents: Vec<Extent> = Vec::new();
    for seg in segments {
        let mut vcn = seg.lowest_vcn;
        for run in &seg.runs {
            let lcn = run
                .lcn
                .ok_or(NtfsError::BadRecord("$MFT con tramos dispersos"))?;
            let ext = Extent {
                mft_offset: vcn * bpc,
                disk_offset: lcn * bpc,
                len: run.clusters * bpc,
            };
            match extents.last_mut() {
                Some(prev)
                    if prev.mft_offset + prev.len == ext.mft_offset
                        && prev.disk_offset + prev.len == ext.disk_offset =>
                {
                    prev.len += ext.len
                }
                _ => extents.push(ext),
            }
            vcn += run.clusters;
        }
    }
    Ok(extents)
}

/// Lee el contenido completo de un atributo no residente (en orden de VCN).
fn read_segments<R: ReadAt + ?Sized>(
    vol: &R,
    boot: &BootSector,
    segments: &[&NonResidentAttr],
) -> Result<Vec<u8>, NtfsError> {
    let bpc = u64::from(boot.bytes_per_cluster);
    let size = segments.first().map_or(0, |s| s.data_size) as usize;
    let mut out = Vec::with_capacity(size);
    'outer: for seg in segments {
        for run in &seg.runs {
            let start = out.len();
            out.resize(start + (run.clusters * bpc) as usize, 0);
            if let Some(lcn) = run.lcn {
                vol.read_at(lcn * bpc, &mut out[start..])?;
            }
            if out.len() >= size {
                break 'outer;
            }
        }
    }
    out.truncate(size);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_bitmap_splits_by_chunk() {
        assert_eq!(
            plan_chunks(&[0xFF, 0xFF], 10, 4, 8),
            vec![0..4, 4..8, 8..10]
        );
    }

    #[test]
    fn large_gaps_are_skipped() {
        let mut bitmap = vec![0u8; 25];
        bitmap[0] = 0b0000_0111; // registros 0..3
        bitmap[12] = 0b0111_0000; // registros 100..103
        assert_eq!(plan_chunks(&bitmap, 200, 64, 8), vec![0..3, 100..103]);
    }

    #[test]
    fn small_gaps_are_read_through() {
        // Registros 0..3 y 5..7: el hueco de 2 se lee para no partir la lectura.
        assert_eq!(plan_chunks(&[0b0110_0111], 8, 64, 8), vec![0..7]);
    }

    #[test]
    fn records_past_bitmap_count_as_used() {
        assert_eq!(plan_chunks(&[0b0000_0001], 12, 64, 0), vec![0..1, 8..12]);
    }

    #[test]
    fn empty_bitmap_reads_nothing() {
        assert!(plan_chunks(&[0, 0], 16, 64, 8).is_empty());
    }

    /// Volumen sintético: la `$MFT` tiene dos tramos y el segundo solo está
    /// descrito en un registro de extensión (vía `$ATTRIBUTE_LIST`).
    #[test]
    fn mft_extents_listed_in_extension_record() {
        use crate::testutil::{attr_list_entry, RecordBuilder};
        use crate::NS_WIN32;

        const CLUSTER: usize = 4096;
        let mut vol = vec![0u8; 16 * CLUSTER];
        let boot = &mut vol[..512];
        boot[3..11].copy_from_slice(b"NTFS    ");
        boot[0x0B..0x0D].copy_from_slice(&512u16.to_le_bytes());
        boot[0x0D] = 8;
        boot[0x30..0x38].copy_from_slice(&4u64.to_le_bytes());
        boot[0x40] = (-10i8) as u8;

        // Registro 0: tramo VCN 0 (LCN 4 = registros 0..3) y lista de atributos.
        let mut list = attr_list_entry(ATTR_DATA, 0, 0, "");
        list.extend(attr_list_entry(ATTR_DATA, 1, 2, ""));
        let rec0 = RecordBuilder::new(1024)
            .resident(0x20, "", &list)
            .nonresident(
                ATTR_DATA,
                "",
                0,
                2 * CLUSTER as u64,
                &[0x11, 0x01, 0x04, 0x00],
            )
            .build();
        // Registro 2: segundo tramo, VCN 1 en LCN 10 (registros 4..7).
        let rec2 = RecordBuilder::new(1024)
            .base(0)
            .nonresident(ATTR_DATA, "", 1, 0, &[0x11, 0x01, 0x0A, 0x00])
            .build();
        let rec5 = RecordBuilder::new(1024)
            .file_name(5, 5, NS_WIN32, "lejos.txt")
            .build();
        let at = |lcn: usize, rec: usize| lcn * CLUSTER + rec * 1024;
        vol[at(4, 0)..at(4, 1)].copy_from_slice(&rec0);
        vol[at(4, 2)..at(4, 3)].copy_from_slice(&rec2);
        vol[at(10, 1)..at(10, 2)].copy_from_slice(&rec5);

        let layout = read_layout(vol.as_slice()).unwrap();
        assert_eq!(layout.record_count, 8);
        assert_eq!(
            layout.extents,
            vec![
                Extent {
                    mft_offset: 0,
                    disk_offset: 4 * CLUSTER as u64,
                    len: CLUSTER as u64
                },
                Extent {
                    mft_offset: CLUSTER as u64,
                    disk_offset: 10 * CLUSTER as u64,
                    len: CLUSTER as u64
                },
            ]
        );
        let scan = read_records(
            vol.as_slice(),
            &layout,
            &ReadOptions {
                threads: 2,
                chunk_bytes: 2048,
            },
        )
        .unwrap();
        let far = scan.records.iter().find(|r| r.frs == 5).unwrap();
        assert_eq!(far.names[0].name, "lejos.txt");
    }
}
