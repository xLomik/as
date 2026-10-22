//! Salida de resultados: pantalla, CSV y estadísticas.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::time::Duration;

use lince_core::{format_filetime_local, Index};

use crate::sources::{BuildStats, Method};

/// Un resultado: (índice de la fuente, entrada).
pub type Hit = (usize, u32);

pub fn print_paths(indexes: &[Index], hits: &[Hit]) -> io::Result<()> {
    let mut out = BufWriter::with_capacity(1 << 16, io::stdout().lock());
    for &(src, i) in hits {
        writeln!(out, "{}", indexes[src].path(i))?;
    }
    out.flush()
}

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

/// 1234567 → "1.234.567".
pub fn thousands(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (k, ch) in digits.chars().enumerate() {
        if k > 0 && (digits.len() - k).is_multiple_of(3) {
            out.push('.');
        }
        out.push(ch);
    }
    out
}

/// Número con coma decimal, como se escribe en español.
fn decimal(x: f64, places: usize) -> String {
    format!("{x:.places$}").replace('.', ",")
}

pub fn seconds(d: Duration) -> String {
    decimal(d.as_secs_f64(), 3) + " s"
}

pub fn report_build(s: &BuildStats) {
    let what = match &s.method {
        Method::Mft {
            drive,
            records,
            bytes_read,
            bad_records,
        } => {
            let mb = *bytes_read as f64 / (1024.0 * 1024.0);
            let speed = mb / s.elapsed.as_secs_f64().max(1e-9);
            let drive = drive.map_or(String::new(), |d| format!(", disco {}", d.label()));
            let bad = if *bad_records > 0 {
                format!(", {} registros dañados", thousands(*bad_records))
            } else {
                String::new()
            };
            format!(
                "MFT{drive}: {} registros, {} MB leídos ({} MB/s){bad}",
                thousands(*records as u64),
                decimal(mb, 1),
                decimal(speed, 0)
            )
        }
        Method::Walk { errors } => {
            let errs = if *errors > 0 {
                format!(", {} carpetas sin acceso", thousands(*errors))
            } else {
                String::new()
            };
            format!("recorrido de carpetas{errs}")
        }
    };
    eprintln!(
        "[{}] {what}; {} entradas en {}",
        s.label,
        thousands(s.entries as u64),
        seconds(s.elapsed)
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thousands_separator() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(999), "999");
        assert_eq!(thousands(1000), "1.000");
        assert_eq!(thousands(1_234_567), "1.234.567");
    }

    #[test]
    fn spanish_decimals() {
        assert_eq!(decimal(1.26, 1), "1,3");
        assert_eq!(seconds(Duration::from_millis(1500)), "1,500 s");
    }

    #[test]
    fn csv_quoting() {
        assert_eq!(csv_field("C:\\a.txt"), "C:\\a.txt");
        assert_eq!(csv_field("a;b"), "\"a;b\"");
        assert_eq!(csv_field("di \"x\""), "\"di \"\"x\"\"\"");
    }
}
