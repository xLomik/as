//! Acceso a las imágenes NTFS de `lince/fixtures` (generadas con
//! `tools/gen-fixtures.sh`). Solo se usa desde las pruebas.

use std::io::Read;
use std::path::PathBuf;

/// Entrada del listado esperado de una imagen.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Expected {
    /// Ruta relativa a la raíz, con `/` como separador.
    pub path: String,
    pub size: u64,
    pub is_dir: bool,
    /// Fecha de modificación en segundos Unix (truncada).
    pub mtime: i64,
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

/// Devuelve la imagen descomprimida en memoria.
pub fn image(name: &str) -> Vec<u8> {
    let path = fixtures_dir().join(format!("{name}.img.gz"));
    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(file)
        .read_to_end(&mut out)
        .expect("imagen gzip corrupta");
    out
}

/// Ruta en disco de la imagen descomprimida (se crea una vez por proceso).
pub fn image_file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("lince-fixtures-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.img"));
    if !path.exists() {
        let tmp = dir.join(format!("{name}.img.tmp-{:?}", std::thread::current().id()));
        std::fs::write(&tmp, image(name)).unwrap();
        std::fs::rename(&tmp, &path).unwrap();
    }
    path
}

/// Listado esperado, ordenado por ruta.
pub fn expected(name: &str) -> Vec<Expected> {
    let path = fixtures_dir().join(format!("{name}.expected.tsv"));
    let text = std::fs::read_to_string(&path).unwrap();
    let mut out: Vec<Expected> = text
        .lines()
        .map(|line| {
            let f: Vec<&str> = line.split('\t').collect();
            Expected {
                path: f[0].to_string(),
                size: f[1].parse().unwrap(),
                is_dir: f[2] == "D",
                mtime: f[3].parse().unwrap(),
            }
        })
        .collect();
    out.sort();
    out
}
