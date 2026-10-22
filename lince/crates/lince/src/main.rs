//! `lince`: busca archivos por nombre al instante leyendo la MFT de NTFS.

mod mem;
mod output;
mod sources;

use std::cmp::Ordering;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use clap::{Parser, ValueEnum};
use lince_core::{parse_query, search, sort_results, Index, SortKey};
use rayon::prelude::*;

use output::{report_build, seconds, thousands, Hit};
use sources::Source;

const SYNTAX: &str = "\
Sintaxis de búsqueda:
  texto            el nombre contiene «texto» (sin distinguir mayúsculas)
  a b              contiene a y b          \"a b\"   frase exacta
  !texto           no contiene «texto»
  *.pdf  f?to*     comodines sobre el nombre completo
  ext:pdf;docx     extensión
  size:>10mb       tamaño (también <, >=, <=, 1kb..2mb)
  dm:2026-10       modificado en ese periodo (AAAA, AAAA-MM, AAAA-MM-DD,
                   DD/MM/AAAA, >=2026-01-01, 2026-01..2026-03); dc: creado
  file:  folder:   solo archivos / solo carpetas
  path:texto       la ruta completa contiene «texto»
  regex:expr       expresión regular sobre el nombre

Códigos de salida: 0 hay resultados, 1 ninguno, 2 error.";

#[derive(Parser)]
#[command(
    name = "lince",
    version,
    about = "Busca archivos por nombre al instante leyendo la MFT de NTFS.",
    after_help = SYNTAX
)]
struct Args {
    /// Texto a buscar (vacío = todo).
    query: Vec<String>,

    /// Volumen NTFS a leer, p. ej. C: (se puede repetir). Por defecto, todos
    /// los volúmenes NTFS fijos. Requiere administrador.
    #[arg(short = 'v', long = "volume", value_name = "X:")]
    volumes: Vec<String>,

    /// Carpeta a recorrer en vez de leer la MFT: unidades de red, FAT/exFAT
    /// o sin administrador (se puede repetir).
    #[arg(short = 'p', long = "path", value_name = "CARPETA")]
    paths: Vec<PathBuf>,

    /// Imagen de un volumen NTFS (pruebas o análisis forense).
    #[arg(long, value_name = "ARCHIVO")]
    image: Vec<PathBuf>,

    /// Máximo de resultados.
    #[arg(short = 'n', long, value_name = "N")]
    limit: Option<usize>,

    /// Orden de los resultados.
    #[arg(short, long, value_enum, default_value_t = Sort::Path)]
    sort: Sort,

    /// Orden descendente.
    #[arg(long)]
    desc: bool,

    /// Guarda los resultados en un CSV para Excel en vez de mostrarlos.
    #[arg(long, value_name = "ARCHIVO")]
    csv: Option<PathBuf>,

    /// Muestra solo cuántos resultados hay.
    #[arg(long)]
    count: bool,

    /// Muestra tiempos y cifras (en la salida de errores).
    #[arg(long)]
    stats: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum Sort {
    /// Sin ordenar (lo más rápido).
    None,
    Name,
    Path,
    Size,
    Date,
}

fn main() -> ExitCode {
    let args = Args::parse();
    match run(&args) {
        Ok(found) if found > 0 => ExitCode::SUCCESS,
        Ok(_) => ExitCode::from(1),
        Err(msg) => {
            eprintln!("lince: {msg}");
            ExitCode::from(2)
        }
    }
}

/// Une los argumentos respetando los que traían espacios (frases).
fn query_text(parts: &[String]) -> String {
    parts
        .iter()
        .map(|p| {
            if p.contains(char::is_whitespace) && !p.contains('"') {
                format!("\"{p}\"")
            } else {
                p.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn sources(args: &Args) -> Result<Vec<Source>, String> {
    let mut out: Vec<Source> = args.volumes.iter().cloned().map(Source::Volume).collect();
    out.extend(args.image.iter().cloned().map(Source::Image));
    out.extend(args.paths.iter().cloned().map(Source::Path));
    if out.is_empty() {
        #[cfg(windows)]
        out.extend(lince_core::ntfs_volumes().into_iter().map(Source::Volume));
        #[cfg(not(windows))]
        return Err("indique dónde buscar con --path <CARPETA> o --image <ARCHIVO>".into());
    }
    if out.is_empty() {
        return Err("no se encontraron volúmenes NTFS; use --path <CARPETA>".into());
    }
    Ok(out)
}

fn run(args: &Args) -> Result<usize, String> {
    let query = parse_query(&query_text(&args.query)).map_err(|e| e.to_string())?;
    let sources = sources(args)?;

    // Todos los volúmenes a la vez: cada uno con su propia cola de lecturas.
    let warn = |m: String| eprintln!("lince: aviso: {m}");
    let built: Vec<_> = sources.par_iter().map(|s| s.build(&warn)).collect();
    let mut indexes = Vec::new();
    for b in built {
        match b {
            Ok(b) => {
                if args.stats {
                    report_build(&b.stats);
                }
                indexes.push(b.index);
            }
            Err(e) => eprintln!("lince: {e}"),
        }
    }
    if indexes.is_empty() {
        return Err("no se pudo leer ninguna fuente".into());
    }

    let t = Instant::now();
    let mut hits: Vec<Hit> = indexes
        .iter()
        .enumerate()
        .flat_map(|(s, ix)| search(ix, &query).into_iter().map(move |i| (s, i)))
        .collect();
    let searched = t.elapsed();
    if !args.count {
        sort_hits(&indexes, &mut hits, args.sort, args.desc);
    }
    let total = hits.len();
    if let Some(n) = args.limit {
        hits.truncate(n);
    }
    if args.stats {
        eprintln!(
            "búsqueda: {} resultados en {} (orden: {})",
            thousands(total as u64),
            seconds(searched),
            seconds(t.elapsed() - searched)
        );
        if let Some(peak) = mem::peak_memory_bytes() {
            eprintln!("memoria máxima: {} MB", thousands(peak >> 20));
        }
    }

    if args.count {
        println!("{}", hits.len());
    } else if let Some(path) = &args.csv {
        output::write_csv(path, &indexes, &hits).map_err(|e| format!("{}: {e}", path.display()))?;
    } else if let Err(e) = output::print_paths(&indexes, &hits) {
        // Salida cerrada (p. ej. `| head`): no es un error del programa.
        if e.kind() != std::io::ErrorKind::BrokenPipe {
            return Err(e.to_string());
        }
    }
    Ok(hits.len())
}

fn sort_hits(indexes: &[Index], hits: &mut Vec<Hit>, sort: Sort, desc: bool) {
    let key = match sort {
        Sort::None => return,
        Sort::Name => SortKey::Name,
        Sort::Path => SortKey::Path,
        Sort::Size => SortKey::Size,
        Sort::Date => SortKey::Modified,
    };
    if indexes.len() == 1 {
        let mut ids: Vec<u32> = hits.iter().map(|&(_, i)| i).collect();
        sort_results(&indexes[0], &mut ids, key, desc);
        *hits = ids.into_iter().map(|i| (0, i)).collect();
        return;
    }
    if let SortKey::Path = key {
        // Por ruta: cada fuente por separado y las fuentes por su etiqueta.
        let mut order: Vec<usize> = (0..indexes.len()).collect();
        order.sort_by(|&a, &b| indexes[a].label().cmp(indexes[b].label()));
        if desc {
            order.reverse();
        }
        let mut out = Vec::with_capacity(hits.len());
        for s in order {
            let mut ids: Vec<u32> = hits.iter().filter(|h| h.0 == s).map(|h| h.1).collect();
            sort_results(&indexes[s], &mut ids, key, desc);
            out.extend(ids.into_iter().map(|i| (s, i)));
        }
        *hits = out;
        return;
    }
    let name = |&(s, i): &Hit| indexes[s].folded_name(i);
    let cmp = |a: &Hit, b: &Hit| -> Ordering {
        let (ea, eb) = (indexes[a.0].entry(a.1), indexes[b.0].entry(b.1));
        let primary = match key {
            SortKey::Size => ea.size.cmp(&eb.size),
            SortKey::Modified => ea.modified.cmp(&eb.modified),
            _ => Ordering::Equal,
        };
        primary.then_with(|| name(a).cmp(name(b))).then(a.cmp(b))
    };
    hits.par_sort_by(cmp);
    if desc {
        hits.reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_keeps_phrases() {
        let parts = vec!["foto vac".to_string(), "ext:jpg".to_string()];
        assert_eq!(query_text(&parts), "\"foto vac\" ext:jpg");
    }
}
