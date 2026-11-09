//! Word 97–2003 (.doc): un archivo compuesto de OLE cuyo texto está
//! repartido en «piezas» (tabla de piezas, especificación MS-DOC). Cada
//! pieza va en Windows-1252 (comprimida) o en UTF-16.

use std::io::{Read, Seek};
use std::path::Path;

use encoding_rs::{UTF_16LE, WINDOWS_1252};

use super::{err, ContentError};

pub fn read(path: &Path) -> Result<String, ContentError> {
    let mut file = cfb::open(path).map_err(err)?;
    let word = stream(&mut file, "/WordDocument")?;
    let fib = Fib::parse(&word)?;
    let table = stream(&mut file, if fib.table1 { "/1Table" } else { "/0Table" })?;
    pieces_text(&word, &table, fib.fc_clx, fib.lcb_clx)
}

fn stream<F: Read + Seek>(
    file: &mut cfb::CompoundFile<F>,
    name: &str,
) -> Result<Vec<u8>, ContentError> {
    let mut s = file.open_stream(name).map_err(err)?;
    let mut out = Vec::new();
    s.read_to_end(&mut out).map_err(err)?;
    Ok(out)
}

fn u16_at(b: &[u8], off: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(off..off + 2)?.try_into().ok()?))
}

fn u32_at(b: &[u8], off: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(off..off + 4)?.try_into().ok()?))
}

/// Cabecera del documento (FIB): lo justo para hallar la tabla de piezas.
struct Fib {
    /// La tabla está en `1Table` (si no, en `0Table`).
    table1: bool,
    fc_clx: u32,
    lcb_clx: u32,
}

impl Fib {
    fn parse(w: &[u8]) -> Result<Fib, ContentError> {
        let bad = || err("no es un documento de Word 97-2003");
        if u16_at(w, 0) != Some(0xA5EC) {
            return Err(bad());
        }
        let flags = u16_at(w, 0x0A).ok_or_else(bad)?;
        if flags & 0x0100 != 0 {
            return Err(err("documento protegido con contraseña"));
        }
        // FibBase (32 bytes), luego tres listas con su tamaño delante:
        // FibRgW (u16), FibRgLw (u32) y FibRgFcLcb (pares u32).
        let csw = usize::from(u16_at(w, 32).ok_or_else(bad)?);
        let lw = 34 + csw * 2;
        let cslw = usize::from(u16_at(w, lw).ok_or_else(bad)?);
        let pairs_at = lw + 2 + cslw * 4;
        let pairs = usize::from(u16_at(w, pairs_at).ok_or_else(bad)?);
        // fcClx/lcbClx es el par 33 de FibRgFcLcb97.
        const CLX: usize = 33;
        if pairs <= CLX {
            return Err(bad());
        }
        let at = pairs_at + 2 + CLX * 8;
        Ok(Fib {
            table1: flags & 0x0200 != 0,
            fc_clx: u32_at(w, at).ok_or_else(bad)?,
            lcb_clx: u32_at(w, at + 4).ok_or_else(bad)?,
        })
    }
}

fn pieces_text(word: &[u8], table: &[u8], fc: u32, lcb: u32) -> Result<String, ContentError> {
    let bad = || err("tabla de piezas dañada");
    let (fc, lcb) = (fc as usize, lcb as usize);
    let clx = table
        .get(fc..fc.checked_add(lcb).ok_or_else(bad)?)
        .ok_or_else(bad)?;
    // Primero vienen propiedades (Prc, 0x01) que no interesan; luego la
    // tabla de piezas (Pcdt, 0x02).
    let mut i = 0;
    while clx.get(i) == Some(&0x01) {
        let size = u16_at(clx, i + 1).ok_or_else(bad)? as i16;
        i += 3 + usize::try_from(size).map_err(|_| bad())?;
    }
    if clx.get(i) != Some(&0x02) {
        return Err(bad());
    }
    let size = u32_at(clx, i + 1).ok_or_else(bad)? as usize;
    let plc = clx.get(i + 5..i + 5 + size).ok_or_else(bad)?;
    // n+1 posiciones de carácter (u32) y n descriptores de pieza (8 bytes).
    let n = size.saturating_sub(4) / 12;
    let mut out = String::new();
    for k in 0..n {
        let (Some(cp0), Some(cp1)) = (u32_at(plc, k * 4), u32_at(plc, (k + 1) * 4)) else {
            break;
        };
        let count = cp1.saturating_sub(cp0) as usize;
        let Some(raw) = u32_at(plc, (n + 1) * 4 + k * 8 + 2) else {
            break;
        };
        let start = (raw & 0x3FFF_FFFF) as usize;
        if raw & 0x4000_0000 != 0 {
            // Comprimida: un byte por carácter a partir de start/2.
            if let Some(b) = word.get(start / 2..start / 2 + count) {
                out.push_str(&WINDOWS_1252.decode_without_bom_handling(b).0);
            }
        } else if let Some(b) = word.get(start..start + count * 2) {
            out.push_str(&UTF_16LE.decode_without_bom_handling(b).0);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Como los .doc de Word: una pieza en Windows-1252 («comprimida») y
    /// otra en UTF-16, con propiedades (Prc) delante de la tabla.
    #[test]
    fn compressed_and_unicode_pieces() {
        let mut word = vec![0u8; 300];
        word[100..105].copy_from_slice(b"Cami\xF3"); // «Camió» en 1252
        let utf16: Vec<u8> = "n 4512".encode_utf16().flat_map(u16::to_le_bytes).collect();
        word[200..200 + utf16.len()].copy_from_slice(&utf16);

        let mut plc = Vec::new();
        for cp in [0u32, 5, 11] {
            plc.extend(cp.to_le_bytes());
        }
        for fc in [(100u32 * 2) | 0x4000_0000, 200] {
            plc.extend([0, 0]);
            plc.extend(fc.to_le_bytes());
            plc.extend([0, 0]);
        }
        let mut table = vec![0xEE; 10]; // basura antes de la tabla
        let fc_clx = table.len() as u32;
        table.extend([0x01, 2, 0, 0xAA, 0xBB]); // Prc de 2 bytes
        table.push(0x02);
        table.extend((plc.len() as u32).to_le_bytes());
        table.extend(&plc);
        let lcb = table.len() as u32 - fc_clx;

        assert_eq!(
            pieces_text(&word, &table, fc_clx, lcb).unwrap(),
            "Camión 4512"
        );
        assert!(pieces_text(&word, &table, fc_clx, lcb + 100).is_err());
    }
}
