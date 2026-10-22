//! Normalización de mayúsculas para comparar nombres sin distinguirlas.

/// Añade a `out` el texto `s` en minúsculas (UTF-8). Rápido para ASCII.
pub fn fold_into(s: &str, out: &mut Vec<u8>) {
    if s.is_ascii() {
        out.extend(s.bytes().map(|b| b.to_ascii_lowercase()));
        return;
    }
    let mut buf = [0u8; 4];
    for ch in s.chars() {
        for lower in ch.to_lowercase() {
            out.extend_from_slice(lower.encode_utf8(&mut buf).as_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folded(s: &str) -> String {
        let mut out = Vec::new();
        fold_into(s, &mut out);
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn ascii_lowercases() {
        assert_eq!(folded("MAYUS.TXT"), "mayus.txt");
    }

    #[test]
    fn unicode_lowercases() {
        assert_eq!(folded("CANCIÓN Ñ.TXT"), "canción ñ.txt");
        assert_eq!(folded("ΑΒΓ"), "αβγ");
    }

    #[test]
    fn appends_without_clearing() {
        let mut out = b"x\0".to_vec();
        fold_into("AB", &mut out);
        assert_eq!(out, b"x\0ab");
    }
}
