//! Construcción de índices desde volúmenes, imágenes o carpetas.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::{index_from_mft, walk, DriveKind, Index};
use lince_ntfs::{read_layout, read_records, NtfsError, ReadAt, ReadOptions};

/// De dónde sale un índice.
pub enum Source {
    /// Volumen de Windows (`C:`).
    Volume(String),
    Image(PathBuf),
    Path(PathBuf),
}

/// Cifras de cómo se construyó un índice (para `--stats`).
pub struct BuildStats {
    pub label: String,
    pub method: Method,
    pub entries: usize,
    pub elapsed: Duration,
}

pub enum Method {
    Mft {
        drive: Option<DriveKind>,
        records: usize,
        bytes_read: u64,
        bad_records: u64,
    },
    Walk {
        errors: u64,
    },
}

pub struct Built {
    pub index: Index,
    pub stats: BuildStats,
}

impl Source {
    pub fn label(&self) -> String {
        match self {
            Source::Volume(v) => {
                let v = v.trim_end_matches(['\\', '/', ':']).to_uppercase();
                format!("{v}:")
            }
            Source::Image(_) => "IMG:".to_string(),
            Source::Path(p) => p.display().to_string(),
        }
    }

    #[cfg_attr(not(windows), allow(unused_variables))]
    /// Construye el índice. `warn` recibe avisos para el usuario (p. ej.
    /// cuando se recurre al recorrido de carpetas por falta de permisos).
    pub fn build(&self, warn: &dyn Fn(String)) -> Result<Built, String> {
        let start = Instant::now();
        let label = self.label();
        match self {
            Source::Image(path) => {
                let file =
                    std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
                from_mft(&label, &file, None, start).map_err(|e| e.to_string())
            }
            Source::Path(path) => from_walk(&label, path.clone(), start),
            #[cfg(windows)]
            Source::Volume(_) => match crate::Volume::open(&label) {
                Ok(vol) => {
                    let kind = crate::drive_kind(&label);
                    match from_mft(&label, &vol, Some(kind), start) {
                        Err(NtfsError::NotNtfs) => {
                            warn(format!("{label} no es NTFS; se recorren sus carpetas."));
                            from_walk(&label, PathBuf::from(format!("{label}\\")), start)
                        }
                        other => other.map_err(|e| e.to_string()),
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    warn(format!(
                        "sin permisos de administrador para leer la MFT de {label}; se recorren \
                         sus carpetas (mucho más lento). Abra la consola como administrador para \
                         la máxima velocidad."
                    ));
                    from_walk(&label, PathBuf::from(format!("{label}\\")), start)
                }
                // Unidades de red y otras que no admiten lectura cruda.
                Err(e) => {
                    warn(format!(
                        "no se pudo leer la MFT de {label} ({e}); se recorren sus carpetas."
                    ));
                    from_walk(&label, PathBuf::from(format!("{label}\\")), start)
                }
            },
            #[cfg(not(windows))]
            Source::Volume(_) => Err(format!(
                "{label}: los volúmenes solo se pueden leer en Windows"
            )),
        }
    }
}

fn from_mft<R: ReadAt + ?Sized>(
    label: &str,
    vol: &R,
    drive: Option<DriveKind>,
    start: Instant,
) -> Result<Built, NtfsError> {
    let opts = drive.map_or(
        ReadOptions {
            threads: 4,
            chunk_bytes: 2 << 20,
        },
        DriveKind::read_options,
    );
    let layout = read_layout(vol)?;
    let scan = read_records(vol, &layout, &opts)?;
    let (records, bytes_read, bad_records) =
        (scan.records.len(), scan.bytes_read, scan.bad_records);
    let index = index_from_mft(label, scan);
    Ok(Built {
        stats: BuildStats {
            label: label.to_string(),
            method: Method::Mft {
                drive,
                records,
                bytes_read,
                bad_records,
            },
            entries: index.len().saturating_sub(1),
            elapsed: start.elapsed(),
        },
        index,
    })
}

fn from_walk(label: &str, path: PathBuf, start: Instant) -> Result<Built, String> {
    let (index, ws) = walk(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(Built {
        stats: BuildStats {
            label: label.to_string(),
            method: Method::Walk { errors: ws.errors },
            entries: index.len().saturating_sub(1),
            elapsed: start.elapsed(),
        },
        index,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_labels_are_normalized() {
        for (given, want) in [("c", "C:"), ("C:", "C:"), ("d:\\", "D:"), ("e:/", "E:")] {
            assert_eq!(Source::Volume(given.into()).label(), want, "{given}");
        }
    }
}
