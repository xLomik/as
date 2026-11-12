//! Construcción de índices desde volúmenes, imágenes o carpetas.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::{index_from_mft, walk, walk_with_threads, DriveKind, Index};
use lince_ntfs::{read_layout, read_records, NtfsError, ReadAt, ReadOptions};

/// Listados de carpeta simultáneos al recorrer una unidad de red.
const NETWORK_WALK_THREADS: usize = 32;

/// ¿La ruta está en otro equipo? (`\\servidor\recurso` o, en Windows, una
/// letra de unidad de red como `Z:`).
pub fn is_network_path(path: &Path) -> bool {
    let s = path.as_os_str().to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return rest.starts_with(r"UNC\");
    }
    if s.starts_with(r"\\.\") {
        return false;
    }
    if s.starts_with(r"\\") || s.starts_with("//") {
        return true;
    }
    #[cfg(windows)]
    {
        crate::windows::is_remote_drive(path)
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Aviso de que un disco se recorre carpeta por carpeta porque Lince no
/// tiene permisos de administrador.
pub fn no_admin_warning(label: &str) -> String {
    format!(
        "{label}: sin permisos de administrador, se recorren sus carpetas (más lento). Para la \
         máxima velocidad abra Lince aceptando la pregunta de Windows (permisos de \
         administrador)."
    )
}

pub fn is_no_admin_warning(warning: &str) -> bool {
    warning.contains(": sin permisos de administrador, se recorren")
}

/// De dónde sale un índice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Volumen de Windows (`C:`).
    Volume(String),
    Image(PathBuf),
    Path(PathBuf),
    /// Unidad de red con letra (`Z:`) y su ruta real (`\\servidor\carpeta`):
    /// se lee por la ruta real, que también ve un proceso con permisos de
    /// administrador (la letra no), y se muestra con la letra.
    Network {
        letter: String,
        unc: PathBuf,
    },
}

/// Cifras de cómo se construyó un índice (para `--stats`).
#[derive(Debug, Clone)]
pub struct BuildStats {
    pub label: String,
    pub method: Method,
    pub entries: usize,
    pub elapsed: Duration,
}

#[derive(Debug, Clone)]
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
            Source::Network { letter, .. } => {
                format!("{}:", letter.trim_end_matches(':').to_uppercase())
            }
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
            Source::Network { unc, .. } => {
                let (index, ws) =
                    crate::walk_as(unc, &label, NETWORK_WALK_THREADS).map_err(|e| {
                        format!("{label} ({}) no está disponible ahora: {e}", unc.display())
                    })?;
                Ok(Built {
                    stats: BuildStats {
                        label: label.clone(),
                        method: Method::Walk { errors: ws.errors },
                        entries: index.len().saturating_sub(1),
                        elapsed: start.elapsed(),
                    },
                    index,
                })
            }
            #[cfg(windows)]
            Source::Volume(_) => match crate::Volume::open(&label) {
                Ok(vol) => {
                    let kind = crate::drive_kind(&label);
                    match from_mft(&label, &vol, Some(kind), start) {
                        Ok(built) => Ok(built),
                        Err(NtfsError::NotNtfs) => {
                            warn(format!("{label} no es NTFS; se recorren sus carpetas."));
                            from_walk(&label, PathBuf::from(format!("{label}\\")), start)
                        }
                        // Mejor lento que nada: un volumen raro no se pierde.
                        Err(e) => {
                            warn(format!(
                                "no se pudo leer la MFT de {label} ({e}); se recorren sus carpetas."
                            ));
                            from_walk(&label, PathBuf::from(format!("{label}\\")), start)
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                    warn(no_admin_warning(&label));
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
    let walked = if is_network_path(&path) {
        walk_with_threads(&path, NETWORK_WALK_THREADS)
    } else {
        walk(&path)
    };
    let (index, ws) = walked.map_err(|e| format!("{}: {e}", path.display()))?;
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
    fn unc_paths_are_network() {
        assert!(is_network_path(Path::new(r"\\servidor\datos")));
        assert!(is_network_path(Path::new("//servidor/datos")));
        assert!(!is_network_path(Path::new(r"C:\datos")));
        assert!(!is_network_path(Path::new("/home/datos")));
    }

    /// Una unidad de red se lee por su ruta real (la ven también los
    /// procesos con permisos de administrador) pero se muestra con su letra.
    #[test]
    fn network_drives_read_the_share_and_show_the_letter() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir(tmp.path().join("Planos")).unwrap();
        std::fs::write(tmp.path().join("Planos").join("casa.dwg"), b"x").unwrap();
        let src = Source::Network {
            letter: "z".into(),
            unc: tmp.path().to_path_buf(),
        };
        assert_eq!(src.label(), "Z:");
        let built = src.build(&|_| {}).unwrap();
        let ix = &built.index;
        assert_eq!(ix.label(), "Z:");
        let i = (0..ix.len() as u32)
            .find(|&i| ix.name(i) == "casa.dwg")
            .unwrap();
        let sep = std::path::MAIN_SEPARATOR;
        assert_eq!(ix.path(i), format!("Z:{sep}Planos{sep}casa.dwg"));
    }

    #[test]
    fn an_unreachable_network_drive_says_so() {
        let src = Source::Network {
            letter: "Z:".into(),
            unc: "/no/existe/esta/carpeta".into(),
        };
        let e = src.build(&|_| {}).err().unwrap();
        assert!(
            e.starts_with("Z: (/no/existe/esta/carpeta) no está disponible"),
            "{e}"
        );
    }

    #[test]
    fn no_admin_warnings_are_recognised() {
        assert!(is_no_admin_warning(&no_admin_warning("C:")));
        assert!(!is_no_admin_warning(
            "Z: (\\srv\\x) no está disponible ahora"
        ));
    }

    #[test]
    fn volume_labels_are_normalized() {
        for (given, want) in [("c", "C:"), ("C:", "C:"), ("d:\\", "D:"), ("e:/", "E:")] {
            assert_eq!(Source::Volume(given.into()).label(), want, "{given}");
        }
    }
}
