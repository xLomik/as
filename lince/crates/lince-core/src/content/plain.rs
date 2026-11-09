//! Texto plano: UTF-8, UTF-16 (con o sin marca BOM) o ANSI de Windows
//! (Windows-1252, el de los archivos viejos del Bloc de notas en español).

use std::path::Path;

use encoding_rs::{UTF_16BE, UTF_16LE, WINDOWS_1252};

use super::{err, ContentError};

pub fn read(path: &Path) -> Result<String, ContentError> {
    Ok(decode(&std::fs::read(path).map_err(err)?))
}

pub fn decode(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        return String::from_utf8_lossy(rest).into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        return UTF_16LE.decode_without_bom_handling(rest).0.into_owned();
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        return UTF_16BE.decode_without_bom_handling(rest).0.into_owned();
    }
    if let Ok(s) = std::str::from_utf8(bytes) {
        if !looks_utf16le(bytes) {
            return s.to_owned();
        }
    }
    if looks_utf16le(bytes) {
        return UTF_16LE.decode_without_bom_handling(bytes).0.into_owned();
    }
    WINDOWS_1252
        .decode_without_bom_handling(bytes)
        .0
        .into_owned()
}

/// UTF-16 sin BOM: en texto latino, casi todos los bytes impares son cero.
fn looks_utf16le(bytes: &[u8]) -> bool {
    let (units, _) = bytes[..bytes.len().min(4096)].as_chunks::<2>();
    let latin = units.iter().filter(|[lo, hi]| *lo != 0 && *hi == 0).count();
    units.len() >= 4 && latin * 4 >= units.len() * 3
}

#[cfg(test)]
mod tests {
    use super::*;

    fn utf16le(s: &str) -> Vec<u8> {
        s.encode_utf16().flat_map(u16::to_le_bytes).collect()
    }

    #[test]
    fn detects_each_encoding() {
        assert_eq!(decode("camión".as_bytes()), "camión");
        assert_eq!(decode(b"\xEF\xBB\xBFcami\xC3\xB3n"), "camión");
        assert_eq!(decode(b"cami\xF3n"), "camión"); // Windows-1252
        let mut bom = vec![0xFF, 0xFE];
        bom.extend(utf16le("camión"));
        assert_eq!(decode(&bom), "camión");
        assert_eq!(decode(&utf16le("sin marca BOM")), "sin marca BOM");
        let be: Vec<u8> = [0xFE, 0xFF]
            .into_iter()
            .chain("ñu".encode_utf16().flat_map(u16::to_be_bytes))
            .collect();
        assert_eq!(decode(&be), "ñu");
    }
}
