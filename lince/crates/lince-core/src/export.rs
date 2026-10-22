//! Exportación de resultados.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::{format_filetime_local, Hit, Index};

/// CSV para Excel en español: UTF-8 con BOM y `;` como separador.
pub fn write_csv(path: &Path, indexes: &[Index], hits: &[Hit]) -> io::Result<()> {
    let mut out = BufWriter::new(File::create(path)?);
    out.write_all("\u{feff}ruta;tamaño;modificado\r\n".as_bytes())?;
    for &(src, i) in hits {
        let ix = &indexes[src];
        let e = ix.entry(i);
        let size = if ix.is_dir(i) {
            String::new()
        } else {
            e.size.to_string()
        };
        writeln!(
            out,
            "{};{};{}\r",
            csv_field(&ix.path(i)),
            size,
            format_filetime_local(e.modified)
        )?;
    }
    out.flush()
}

fn csv_field(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quoting() {
        assert_eq!(csv_field("C:\\a.txt"), "C:\\a.txt");
        assert_eq!(csv_field("a;b"), "\"a;b\"");
        assert_eq!(csv_field("di \"x\""), "\"di \"\"x\"\"\"");
    }
}
