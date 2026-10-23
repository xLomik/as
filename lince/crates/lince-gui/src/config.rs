//! Carpetas que el usuario agregó para buscar (p. ej. `\\servidor\datos`).
//! Se guardan en un archivo de texto, una ruta por línea.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use lince_core::Source;

const HEADER: &str = "# Carpetas agregadas a Lince (una por línea)";

pub fn load_folders(file: &Path) -> Vec<PathBuf> {
    let Ok(text) = fs::read_to_string(file) else {
        return Vec::new();
    };
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(PathBuf::from)
        .collect()
}

/// Reemplaza la lista (archivo temporal + renombrar: nunca queda a medias).
pub fn save_folders(file: &Path, folders: &[PathBuf]) -> io::Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut text = format!("{HEADER}\r\n");
    for f in folders {
        text.push_str(&f.to_string_lossy());
        text.push_str("\r\n");
    }
    let tmp = file.with_extension("tmp");
    fs::write(&tmp, text)?;
    fs::rename(&tmp, file)
}

/// Agrega una carpeta; `false` si ya estaba (sin distinguir mayúsculas, como
/// Windows).
pub fn add_folder(file: &Path, folder: &Path) -> io::Result<bool> {
    let mut folders = load_folders(file);
    if folders.iter().any(|f| same_path(f, folder)) {
        return Ok(false);
    }
    folders.push(folder.to_path_buf());
    save_folders(file, &folders)?;
    Ok(true)
}

/// Suma a `base` las carpetas guardadas que no estén ya (misma ruta sin
/// distinguir mayúsculas ni barra final).
pub fn with_folders(base: Vec<Source>, folders: &[PathBuf]) -> Vec<Source> {
    let mut out = base;
    for f in folders {
        let present = out.iter().any(|s| match s {
            Source::Path(p) | Source::Image(p) => same_path(p, f),
            Source::Volume(_) => false,
        });
        if !present {
            out.push(Source::Path(f.clone()));
        }
    }
    out
}

fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| {
        p.to_string_lossy()
            .trim_end_matches(['\\', '/'])
            .to_lowercase()
    };
    norm(a) == norm(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_means_no_folders() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_folders(&dir.path().join("no-existe.txt")).is_empty());
    }

    #[test]
    fn folders_round_trip_and_are_not_duplicated() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Lince").join("carpetas.txt");
        assert!(add_folder(&file, Path::new(r"\\srv\datos")).unwrap());
        assert!(add_folder(&file, Path::new(r"D:\Proyectos 2026\ñandú")).unwrap());
        assert!(!add_folder(&file, Path::new(r"\\SRV\Datos")).unwrap());
        assert_eq!(
            load_folders(&file),
            vec![
                PathBuf::from(r"\\srv\datos"),
                PathBuf::from(r"D:\Proyectos 2026\ñandú")
            ]
        );
    }

    #[test]
    fn blank_lines_and_comments_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("carpetas.txt");
        std::fs::write(&file, "# mis carpetas\n\n\\\\srv\\a\r\n  \n").unwrap();
        assert_eq!(load_folders(&file), vec![PathBuf::from(r"\\srv\a")]);
    }

    #[test]
    fn saved_folders_join_the_sources_without_duplicates() {
        let base = vec![
            Source::Volume("C:".into()),
            Source::Path(PathBuf::from(r"\\srv\datos")),
        ];
        let folders = [PathBuf::from(r"\\SRV\Datos\"), PathBuf::from(r"\\srv\otra")];
        assert_eq!(
            with_folders(base, &folders),
            vec![
                Source::Volume("C:".into()),
                Source::Path(PathBuf::from(r"\\srv\datos")),
                Source::Path(PathBuf::from(r"\\srv\otra")),
            ]
        );
    }

    #[test]
    fn clearing_removes_everything() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("carpetas.txt");
        add_folder(&file, Path::new(r"\\srv\a")).unwrap();
        save_folders(&file, &[]).unwrap();
        assert!(load_folders(&file).is_empty());
    }
}
