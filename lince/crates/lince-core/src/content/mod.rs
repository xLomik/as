//! Texto dentro de los archivos: Word, Excel, PDF, OpenDocument y texto
//! plano. Se extrae el texto, se pasa a minúsculas con los espacios
//! unificados y se busca con SIMD, igual que en los nombres.

mod doc;
mod plain;
mod scan;
mod xml;

pub use scan::{content_candidates, content_threads, ContentScan};

use std::fmt;
use std::fs::File;
use std::path::Path;

use memchr::memmem;

/// Tipo de lector según la extensión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Texto plano (UTF-8, UTF-16 o ANSI).
    Text,
    /// Word, PowerPoint modernos (zip con XML).
    OfficeXml,
    /// Writer e Impress de OpenDocument (zip con XML).
    OpenDocument,
    /// Excel nuevo y viejo, y hojas de OpenDocument.
    Spreadsheet,
    /// Word 97–2003 (.doc).
    WordBinary,
    Pdf,
}

/// Archivos que no son de texto ni documentos no se abren.
pub fn format_of(path: &Path) -> Option<Format> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "docx" | "docm" | "dotx" | "dotm" | "pptx" | "pptm" | "potx" | "potm" | "ppsx" | "ppsm" => {
            Format::OfficeXml
        }
        "odt" | "ott" | "odp" | "otp" => Format::OpenDocument,
        "xlsx" | "xlsm" | "xltx" | "xltm" | "xlsb" | "xls" | "xla" | "xlam" | "ods" | "ots" => {
            Format::Spreadsheet
        }
        "doc" | "dot" => Format::WordBinary,
        "pdf" => Format::Pdf,
        e if TEXT_EXTENSIONS.contains(&e) => Format::Text,
        _ => return None,
    })
}

/// Extensiones de archivos de texto (incluidos programas CNC y DXF).
const TEXT_EXTENSIONS: &[&str] = &[
    "txt",
    "text",
    "csv",
    "tsv",
    "log",
    "md",
    "markdown",
    "ini",
    "inf",
    "cfg",
    "conf",
    "config",
    "properties",
    "json",
    "xml",
    "xaml",
    "svg",
    "html",
    "htm",
    "xhtml",
    "css",
    "js",
    "mjs",
    "ts",
    "tsx",
    "jsx",
    "py",
    "rs",
    "go",
    "java",
    "kt",
    "c",
    "h",
    "cc",
    "cpp",
    "hpp",
    "cs",
    "vb",
    "vbs",
    "bas",
    "php",
    "rb",
    "pl",
    "lua",
    "sql",
    "sh",
    "bat",
    "cmd",
    "ps1",
    "psm1",
    "yaml",
    "yml",
    "toml",
    "reg",
    "srt",
    "vtt",
    "tex",
    "nc",
    "ngc",
    "gcode",
    "tap",
    "cnc",
    "mpf",
    "dxf",
    "eml",
];

/// Más que esto no se abre: no es un documento sino un volcado, una base de
/// datos o un vídeo con la extensión cambiada.
pub const MAX_FILE_BYTES: u64 = 256 * 1024 * 1024;

/// Error al leer un archivo, con un mensaje para el usuario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentError(pub String);

impl fmt::Display for ContentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ContentError {}

fn err(e: impl fmt::Display) -> ContentError {
    ContentError(e.to_string())
}

/// Texto del archivo según su extensión.
pub fn extract_text(path: &Path) -> Result<String, ContentError> {
    let format = format_of(path).ok_or_else(|| err("tipo de archivo no admitido"))?;
    let size = std::fs::metadata(path).map_err(err)?.len();
    if size > MAX_FILE_BYTES {
        return Err(err("archivo demasiado grande"));
    }
    // Un lector que se rompe con un archivo raro no debe tumbar la búsqueda.
    std::panic::catch_unwind(|| match format {
        Format::Text => plain::read(path),
        Format::OfficeXml => xml::office(path),
        Format::OpenDocument => xml::open_document(path),
        Format::Spreadsheet => spreadsheet(path),
        Format::WordBinary => doc::read(path),
        Format::Pdf => pdf(path),
    })
    .unwrap_or_else(|_| Err(err("archivo dañado o no reconocido")))
}

/// ¿El archivo contiene todos los textos? Cada uno ya pasado por [`needle`].
pub fn file_contains(path: &Path, needles: &[Vec<u8>]) -> Result<bool, ContentError> {
    let text = normalize(&extract_text(path)?);
    Ok(needles
        .iter()
        .all(|n| n.is_empty() || memmem::find(&text, n).is_some()))
}

/// Texto a buscar, preparado igual que el de los archivos.
pub fn needle(text: &str) -> Vec<u8> {
    let mut n = normalize(text);
    if n.first() == Some(&b' ') {
        n.remove(0);
    }
    if n.last() == Some(&b' ') {
        n.pop();
    }
    n
}

/// Minúsculas y cualquier grupo de espacios, tabuladores o saltos de línea
/// como un solo espacio: una frase se encuentra aunque el documento la
/// parta en dos líneas.
pub fn normalize(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len());
    let mut buf = [0u8; 4];
    for ch in text.chars() {
        if ch.is_whitespace() || ch.is_control() {
            if out.last() != Some(&b' ') {
                out.push(b' ');
            }
        } else if ch.is_ascii() {
            out.push(ch.to_ascii_lowercase() as u8);
        } else {
            for lower in ch.to_lowercase() {
                out.extend_from_slice(lower.encode_utf8(&mut buf).as_bytes());
            }
        }
    }
    out
}

fn spreadsheet(path: &Path) -> Result<String, ContentError> {
    use calamine::{open_workbook_auto, Data, Reader};
    let mut book = open_workbook_auto(path).map_err(err)?;
    let mut out = String::new();
    for name in book.sheet_names() {
        let Ok(range) = book.worksheet_range(&name) else {
            continue;
        };
        for (_, _, cell) in range.used_cells() {
            match cell {
                Data::String(s) => out.push_str(s),
                // Como lo escribe quien usa coma decimal: 4512,75 y 4512.75.
                Data::Float(_) => {
                    let s = cell.to_string();
                    out.push_str(&s);
                    if s.contains('.') {
                        out.push(' ');
                        out.push_str(&s.replace('.', ","));
                    }
                }
                Data::Empty => continue,
                other => out.push_str(&other.to_string()),
            }
            out.push('\n');
        }
    }
    Ok(out)
}

fn pdf(path: &Path) -> Result<String, ContentError> {
    let bytes = std::fs::read(path).map_err(err)?;
    pdf_extract::extract_text_from_mem(&bytes).map_err(err)
}

/// Abre un archivo; el error lleva el motivo legible.
fn open(path: &Path) -> Result<File, ContentError> {
    File::open(path).map_err(err)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_unifies_case_and_spaces() {
        assert_eq!(
            normalize("Orden  de\r\ncompra\t4512"),
            b"orden de compra 4512"
        );
        assert_eq!(normalize("Señal\u{a0}CAMIÓN"), "señal camión".as_bytes());
        assert_eq!(needle("  Orden de  "), b"orden de");
        assert_eq!(normalize(" a \n\n"), b" a ");
    }
}
