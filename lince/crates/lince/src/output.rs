//! Salida de resultados en pantalla y estadísticas.

use std::io::{self, BufWriter, Write};

use lince_core::{decimal, seconds, thousands, BuildStats, Hit, Index, Method};

pub fn print_paths(indexes: &[Index], hits: &[Hit]) -> io::Result<()> {
    let mut out = BufWriter::with_capacity(1 << 16, io::stdout().lock());
    for &(src, i) in hits {
        writeln!(out, "{}", indexes[src].path(i))?;
    }
    out.flush()
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
