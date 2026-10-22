use crate::bytes::{split_ref, u16_at, u32_at, u64_at, u8_at};
use crate::{decode_runs, DataRun, NtfsError};

pub const ATTR_STD_INFO: u32 = 0x10;
pub const ATTR_ATTR_LIST: u32 = 0x20;
pub const ATTR_FILE_NAME: u32 = 0x30;
pub const ATTR_DATA: u32 = 0x80;
pub const ATTR_BITMAP: u32 = 0xB0;

/// Espacios de nombres de `$FILE_NAME`.
pub const NS_POSIX: u8 = 0;
pub const NS_WIN32: u8 = 1;
pub const NS_DOS: u8 = 2;
pub const NS_WIN32_AND_DOS: u8 = 3;

/// Fechas (FILETIME: intervalos de 100 ns desde 1601) y atributos del archivo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StdInfo {
    pub created: u64,
    pub modified: u64,
    pub accessed: u64,
    pub attributes: u32,
}

/// Un nombre del archivo (puede haber varios: hard links).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileName {
    pub parent_frs: u64,
    pub parent_seq: u16,
    pub namespace: u8,
    pub name: String,
}

/// Atributo no residente con su lista de tramos (solo con `keep_runs`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonResidentAttr {
    pub type_code: u32,
    pub named: bool,
    pub lowest_vcn: u64,
    /// Tamaño real; solo es válido en el segmento con `lowest_vcn == 0`.
    pub data_size: u64,
    pub runs: Vec<DataRun>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrListEntry {
    pub type_code: u32,
    pub lowest_vcn: u64,
    /// Registro de la MFT que contiene este atributo.
    pub record_frs: u64,
    pub named: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrList {
    Resident(Vec<AttrListEntry>),
    NonResident { data_size: u64, runs: Vec<DataRun> },
}

/// Lo que nos interesa de un registro FILE en uso.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRecord {
    pub frs: u64,
    pub sequence: u16,
    /// `Some(base)` si es un registro de extensión de otro.
    pub base_frs: Option<u64>,
    pub is_dir: bool,
    pub std_info: Option<StdInfo>,
    /// Nombres, sin los que son solo DOS (8.3).
    pub names: Vec<FileName>,
    /// Tamaño del `$DATA` sin nombre (segmento VCN 0).
    pub data_size: Option<u64>,
    /// Atributos no residentes con sus tramos (solo con `keep_runs`).
    pub nonresident: Vec<NonResidentAttr>,
    pub attr_list: Option<AttrList>,
}

const ATTR_END: u32 = 0xFFFF_FFFF;
const FLAG_IN_USE: u16 = 0x0001;
const FLAG_DIRECTORY: u16 = 0x0002;
/// NTFS protege cada bloque de 512 bytes con el *update sequence array*.
const FIXUP_STRIDE: usize = 512;

/// Restaura los bytes que NTFS sustituye al final de cada sector del registro
/// (*update sequence array*). Falla si algún sector no lleva el número de
/// secuencia esperado: el registro quedó escrito a medias o está corrupto.
pub fn apply_fixups(rec: &mut [u8]) -> Result<(), NtfsError> {
    let usa_off = usize::from(u16_at(rec, 4).ok_or(NtfsError::BadRecord("cabecera corta"))?);
    let usa_count = usize::from(u16_at(rec, 6).ok_or(NtfsError::BadRecord("cabecera corta"))?);
    if usa_count < 2
        || (usa_count - 1) * FIXUP_STRIDE > rec.len()
        || usa_off + 2 * usa_count > rec.len()
    {
        return Err(NtfsError::BadRecord("update sequence array fuera de rango"));
    }
    let usn = [rec[usa_off], rec[usa_off + 1]];
    for i in 0..usa_count - 1 {
        let end = (i + 1) * FIXUP_STRIDE - 2;
        if rec[end..end + 2] != usn {
            return Err(NtfsError::BadRecord("fixup no coincide"));
        }
        let slot = usa_off + 2 + 2 * i;
        rec[end] = rec[slot];
        rec[end + 1] = rec[slot + 1];
    }
    Ok(())
}

/// Interpreta un registro FILE al que ya se le aplicaron los fixups.
///
/// Devuelve `Ok(None)` si el registro está libre o nunca se usó. Con
/// `keep_runs` también guarda los tramos de los atributos no residentes
/// (solo hace falta para la propia `$MFT`).
pub fn parse_record(
    frs: u64,
    rec: &[u8],
    keep_runs: bool,
) -> Result<Option<ParsedRecord>, NtfsError> {
    let sig = rec
        .get(0..4)
        .ok_or(NtfsError::BadRecord("registro corto"))?;
    if sig != b"FILE" {
        return if sig == [0, 0, 0, 0] {
            Ok(None)
        } else {
            Err(NtfsError::BadRecord("firma desconocida"))
        };
    }
    let header = |off| u16_at(rec, off).ok_or(NtfsError::BadRecord("cabecera corta"));
    let flags = header(0x16)?;
    if flags & FLAG_IN_USE == 0 {
        return Ok(None);
    }
    let sequence = header(0x10)?;
    let first_attr = usize::from(header(0x14)?);
    let in_use = u32_at(rec, 0x18).ok_or(NtfsError::BadRecord("cabecera corta"))? as usize;
    let base = split_ref(u64_at(rec, 0x20).ok_or(NtfsError::BadRecord("cabecera corta"))?).0;
    if in_use > rec.len() || first_attr >= in_use {
        return Err(NtfsError::BadRecord("tamaño en uso fuera de rango"));
    }

    let mut out = ParsedRecord {
        frs,
        sequence,
        base_frs: (base != 0).then_some(base),
        is_dir: flags & FLAG_DIRECTORY != 0,
        std_info: None,
        names: Vec::new(),
        data_size: None,
        nonresident: Vec::new(),
        attr_list: None,
    };

    let mut pos = first_attr;
    loop {
        let type_code = u32_at(rec, pos).ok_or(NtfsError::BadRecord("atributo fuera de rango"))?;
        if type_code == ATTR_END {
            break;
        }
        let len = u32_at(rec, pos + 4).unwrap_or(0) as usize;
        if len < 0x18 || pos + len > in_use {
            return Err(NtfsError::BadRecord("atributo fuera de rango"));
        }
        let attr = Attr::new(&rec[pos..pos + len])?;
        match (type_code, attr.resident) {
            (ATTR_STD_INFO, true) => out.std_info = parse_std_info(attr.value()?),
            (ATTR_FILE_NAME, true) => {
                if let Some(name) = parse_file_name(attr.value()?)? {
                    out.names.push(name);
                }
            }
            (ATTR_DATA, true) if !attr.named => {
                out.data_size = Some(attr.value()?.len() as u64);
            }
            (ATTR_DATA, false) if !attr.named && attr.lowest_vcn()? == 0 => {
                out.data_size = Some(attr.data_size()?);
            }
            (ATTR_ATTR_LIST, true) => {
                out.attr_list = Some(AttrList::Resident(parse_attr_list(attr.value()?)?));
            }
            (ATTR_ATTR_LIST, false) => {
                out.attr_list = Some(AttrList::NonResident {
                    data_size: attr.data_size()?,
                    runs: decode_runs(attr.runs()?)?,
                });
            }
            _ => {}
        }
        if keep_runs && !attr.resident {
            out.nonresident.push(NonResidentAttr {
                type_code,
                named: attr.named,
                lowest_vcn: attr.lowest_vcn()?,
                data_size: attr.data_size()?,
                runs: decode_runs(attr.runs()?)?,
            });
        }
        pos += len;
    }
    Ok(Some(out))
}

/// Interpreta el contenido de un `$ATTRIBUTE_LIST`.
pub fn parse_attr_list(bytes: &[u8]) -> Result<Vec<AttrListEntry>, NtfsError> {
    const BAD: NtfsError = NtfsError::BadRecord("lista de atributos inválida");
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        let type_code = u32_at(bytes, pos).ok_or(BAD)?;
        if type_code == 0 || type_code == ATTR_END {
            break;
        }
        let len = usize::from(u16_at(bytes, pos + 4).ok_or(BAD)?);
        if len < 0x1A || pos + len > bytes.len() {
            return Err(BAD);
        }
        out.push(AttrListEntry {
            type_code,
            lowest_vcn: u64_at(bytes, pos + 8).ok_or(BAD)?,
            record_frs: split_ref(u64_at(bytes, pos + 16).ok_or(BAD)?).0,
            named: u8_at(bytes, pos + 6).ok_or(BAD)? > 0,
        });
        pos += len;
    }
    Ok(out)
}

/// Vista de un atributo dentro del registro.
struct Attr<'a> {
    bytes: &'a [u8],
    resident: bool,
    named: bool,
}

impl<'a> Attr<'a> {
    fn new(bytes: &'a [u8]) -> Result<Self, NtfsError> {
        Ok(Attr {
            bytes,
            resident: bytes[8] == 0,
            named: bytes[9] > 0,
        })
    }

    fn value(&self) -> Result<&'a [u8], NtfsError> {
        const BAD: NtfsError = NtfsError::BadRecord("valor residente fuera de rango");
        let len = u32_at(self.bytes, 0x10).ok_or(BAD)? as usize;
        let off = usize::from(u16_at(self.bytes, 0x14).ok_or(BAD)?);
        self.bytes.get(off..off + len).ok_or(BAD)
    }

    fn lowest_vcn(&self) -> Result<u64, NtfsError> {
        u64_at(self.bytes, 0x10).ok_or(NtfsError::BadRecord("atributo no residente corto"))
    }

    fn data_size(&self) -> Result<u64, NtfsError> {
        u64_at(self.bytes, 0x30).ok_or(NtfsError::BadRecord("atributo no residente corto"))
    }

    fn runs(&self) -> Result<&'a [u8], NtfsError> {
        const BAD: NtfsError = NtfsError::BadRecord("data runs fuera de rango");
        let off = usize::from(u16_at(self.bytes, 0x20).ok_or(BAD)?);
        self.bytes.get(off..).ok_or(BAD)
    }
}

fn parse_std_info(v: &[u8]) -> Option<StdInfo> {
    Some(StdInfo {
        created: u64_at(v, 0x00)?,
        modified: u64_at(v, 0x08)?,
        accessed: u64_at(v, 0x18)?,
        attributes: u32_at(v, 0x20)?,
    })
}

fn parse_file_name(v: &[u8]) -> Result<Option<FileName>, NtfsError> {
    const BAD: NtfsError = NtfsError::BadRecord("$FILE_NAME inválido");
    let units = usize::from(u8_at(v, 0x40).ok_or(BAD)?);
    let namespace = u8_at(v, 0x41).ok_or(BAD)?;
    if namespace == NS_DOS {
        return Ok(None);
    }
    let raw = v.get(0x42..0x42 + 2 * units).ok_or(BAD)?;
    let utf16: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&c| u16::from_le_bytes(c))
        .collect();
    let (parent_frs, parent_seq) = split_ref(u64_at(v, 0).ok_or(BAD)?);
    Ok(Some(FileName {
        parent_frs,
        parent_seq,
        namespace,
        name: String::from_utf16_lossy(&utf16),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{attr_list_entry, RecordBuilder};

    const T0: u64 = 133_000_000_000_000_000;

    fn parse(b: &RecordBuilder, keep_runs: bool) -> Option<ParsedRecord> {
        let mut disk = b.build();
        apply_fixups(&mut disk).unwrap();
        parse_record(40, &disk, keep_runs).unwrap()
    }

    #[test]
    fn fixups_restore_sector_ends() {
        let b = RecordBuilder::new(1024)
            .std_info(T0, T0, T0, 0)
            .file_name(5, 5, NS_WIN32, "hola.txt");
        let raw = b.build_raw();
        let mut disk = b.build();
        assert_ne!(disk, raw);
        apply_fixups(&mut disk).unwrap();
        assert_eq!(disk, raw);
    }

    #[test]
    fn fixup_mismatch_is_error() {
        let mut disk = RecordBuilder::new(1024).build();
        disk[1022] ^= 0xFF;
        assert!(matches!(
            apply_fixups(&mut disk),
            Err(NtfsError::BadRecord(_))
        ));
    }

    #[test]
    fn usa_out_of_bounds_is_error() {
        let mut disk = RecordBuilder::new(1024).build();
        disk[6..8].copy_from_slice(&600u16.to_le_bytes());
        assert!(apply_fixups(&mut disk).is_err());
    }

    #[test]
    fn parses_file_with_win32_name_and_resident_data() {
        let b = RecordBuilder::new(1024)
            .sequence(7)
            .std_info(T0, T0 + 10, T0 + 20, 0x20)
            .file_name(5, 5, NS_WIN32, "hola.txt")
            .resident(ATTR_DATA, "", b"hola\n");
        let r = parse(&b, false).unwrap();
        assert_eq!(r.frs, 40);
        assert_eq!(r.sequence, 7);
        assert_eq!(r.base_frs, None);
        assert!(!r.is_dir);
        assert_eq!(
            r.std_info,
            Some(StdInfo {
                created: T0,
                modified: T0 + 10,
                accessed: T0 + 20,
                attributes: 0x20
            })
        );
        assert_eq!(
            r.names,
            vec![FileName {
                parent_frs: 5,
                parent_seq: 5,
                namespace: NS_WIN32,
                name: "hola.txt".into()
            }]
        );
        assert_eq!(r.data_size, Some(5));
        assert_eq!(r.attr_list, None);
    }

    #[test]
    fn skips_dos_only_names() {
        let b = RecordBuilder::new(1024)
            .file_name(5, 1, NS_WIN32, "Archivo largo.txt")
            .file_name(5, 1, NS_DOS, "ARCHIV~1.TXT");
        let r = parse(&b, false).unwrap();
        assert_eq!(r.names.len(), 1);
        assert_eq!(r.names[0].name, "Archivo largo.txt");
    }

    #[test]
    fn keeps_every_hard_link_and_unicode() {
        let b = RecordBuilder::new(1024)
            .file_name(30, 1, NS_POSIX, "Canción ñ.txt")
            .file_name(31, 2, NS_POSIX, "🦊 zorro.txt");
        let r = parse(&b, false).unwrap();
        let names: Vec<_> = r
            .names
            .iter()
            .map(|n| (n.parent_frs, n.parent_seq, n.name.as_str()))
            .collect();
        assert_eq!(
            names,
            vec![(30, 1, "Canción ñ.txt"), (31, 2, "🦊 zorro.txt")]
        );
    }

    #[test]
    fn size_comes_from_unnamed_data_only() {
        let b = RecordBuilder::new(1024)
            .file_name(5, 5, NS_WIN32, "grande.bin")
            .nonresident(ATTR_DATA, "", 0, 3_000_000, &[0x21, 0xB8, 0x0B, 0x40, 0x00])
            .resident(ATTR_DATA, "oculto", b"secreto\n");
        assert_eq!(parse(&b, false).unwrap().data_size, Some(3_000_000));
    }

    #[test]
    fn later_data_segment_has_no_size() {
        let b = RecordBuilder::new(1024).base(77).nonresident(
            ATTR_DATA,
            "",
            100,
            999,
            &[0x11, 0x04, 0x40, 0x00],
        );
        let r = parse(&b, false).unwrap();
        assert_eq!(r.base_frs, Some(77));
        assert_eq!(r.data_size, None);
    }

    #[test]
    fn directory_flag() {
        let b = RecordBuilder::new(1024)
            .directory()
            .file_name(5, 5, NS_WIN32, "Docs");
        let r = parse(&b, false).unwrap();
        assert!(r.is_dir);
        assert_eq!(r.data_size, None);
    }

    #[test]
    fn unused_or_blank_records_are_none() {
        assert_eq!(parse(&RecordBuilder::new(1024).not_in_use(), false), None);
        assert_eq!(parse_record(3, &[0u8; 1024], false).unwrap(), None);
    }

    #[test]
    fn short_std_info_is_accepted() {
        // ntfs-3g escribe $STANDARD_INFORMATION de 48 bytes (formato NTFS 1.2).
        let mut v = vec![0u8; 48];
        v[8..16].copy_from_slice(&T0.to_le_bytes());
        v[32..36].copy_from_slice(&0x20u32.to_le_bytes());
        let b = RecordBuilder::new(1024).resident(ATTR_STD_INFO, "", &v);
        let si = parse(&b, false).unwrap().std_info.unwrap();
        assert_eq!(si.modified, T0);
        assert_eq!(si.attributes, 0x20);
    }

    #[test]
    fn attribute_past_end_is_error() {
        let b = RecordBuilder::new(1024).file_name(5, 5, NS_WIN32, "x");
        let mut disk = b.build();
        apply_fixups(&mut disk).unwrap();
        let first = u16::from_le_bytes([disk[0x14], disk[0x15]]) as usize;
        disk[first + 4..first + 8].copy_from_slice(&5000u32.to_le_bytes());
        assert!(matches!(
            parse_record(40, &disk, false),
            Err(NtfsError::BadRecord(_))
        ));
    }

    #[test]
    fn keep_runs_collects_nonresident_attributes() {
        let b = RecordBuilder::new(1024)
            .nonresident(ATTR_DATA, "", 0, 16384, &[0x11, 0x04, 0x40, 0x00])
            .nonresident(ATTR_BITMAP, "", 0, 8, &[0x11, 0x01, 0x02, 0x00]);
        let kept = parse(&b, true).unwrap().nonresident;
        assert_eq!(
            kept,
            vec![
                NonResidentAttr {
                    type_code: ATTR_DATA,
                    named: false,
                    lowest_vcn: 0,
                    data_size: 16384,
                    runs: vec![DataRun {
                        lcn: Some(0x40),
                        clusters: 4
                    }],
                },
                NonResidentAttr {
                    type_code: ATTR_BITMAP,
                    named: false,
                    lowest_vcn: 0,
                    data_size: 8,
                    runs: vec![DataRun {
                        lcn: Some(0x02),
                        clusters: 1
                    }],
                },
            ]
        );
        assert!(parse(&b, false).unwrap().nonresident.is_empty());
    }

    #[test]
    fn resident_attribute_list_is_parsed() {
        let mut list = attr_list_entry(ATTR_DATA, 0, 0, "");
        list.extend(attr_list_entry(ATTR_DATA, 500, 17, ""));
        let b = RecordBuilder::new(1024).resident(ATTR_ATTR_LIST, "", &list);
        assert_eq!(
            parse(&b, false).unwrap().attr_list,
            Some(AttrList::Resident(vec![
                AttrListEntry {
                    type_code: ATTR_DATA,
                    lowest_vcn: 0,
                    record_frs: 0,
                    named: false
                },
                AttrListEntry {
                    type_code: ATTR_DATA,
                    lowest_vcn: 500,
                    record_frs: 17,
                    named: false
                },
            ]))
        );
    }

    #[test]
    fn truncated_attribute_list_is_error() {
        let list = attr_list_entry(ATTR_DATA, 0, 0, "");
        assert!(parse_attr_list(&list[..10]).is_err());
    }
}
