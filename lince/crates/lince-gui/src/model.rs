//! Textos que muestra la lista de resultados.

use lince_core::{decimal, format_filetime_short, thousands, Hit, Index, SortKey, NO_PARENT};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Name,
    Folder,
    Size,
    Modified,
}

pub const COLUMNS: [Column; 4] = [Column::Name, Column::Folder, Column::Size, Column::Modified];

impl Column {
    pub fn title(self) -> &'static str {
        match self {
            Column::Name => "Nombre",
            Column::Folder => "Carpeta",
            Column::Size => "Tamaño",
            Column::Modified => "Modificado",
        }
    }

    pub fn sort_key(self) -> SortKey {
        match self {
            Column::Name => SortKey::Name,
            Column::Folder => SortKey::Path,
            Column::Size => SortKey::Size,
            Column::Modified => SortKey::Modified,
        }
    }

    /// Ancho inicial en píxeles a 96 ppp.
    pub fn width(self) -> i32 {
        match self {
            Column::Name => 300,
            Column::Folder => 430,
            Column::Size => 90,
            Column::Modified => 130,
        }
    }

    pub fn right_aligned(self) -> bool {
        self == Column::Size
    }
}

const KIB: u64 = 1 << 10;
const MIB: u64 = 1 << 20;
const GIB: u64 = 1 << 30;
const TIB: u64 = 1 << 40;

/// Tamaño legible: bytes, KB redondeados hacia arriba (como el Explorador) y
/// MB/GB/TB con un decimal.
pub fn format_size(bytes: u64) -> String {
    let unit = |div: u64, name: &str| format!("{} {name}", decimal(bytes as f64 / div as f64, 1));
    match bytes {
        1 => "1 byte".to_string(),
        b if b < KIB => format!("{} bytes", thousands(b)),
        b if b < MIB => format!("{} KB", thousands(b.div_ceil(KIB))),
        b if b < GIB => unit(MIB, "MB"),
        b if b < TIB => unit(GIB, "GB"),
        _ => unit(TIB, "TB"),
    }
}

/// Texto de una celda de la lista.
pub fn cell_text<I: AsRef<Index>>(indexes: &[I], (src, i): Hit, col: Column) -> String {
    let ix = indexes[src].as_ref();
    match col {
        Column::Name => ix.name(i).to_string(),
        Column::Folder => match ix.entry(i).parent {
            NO_PARENT => format!("{}{}?", ix.label(), ix.separator()),
            p => ix.path(p),
        },
        Column::Size if ix.is_dir(i) => String::new(),
        Column::Size => format_size(ix.entry(i).size),
        Column::Modified => format_filetime_short(ix.entry(i).modified),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_like_the_explorer() {
        assert_eq!(format_size(0), "0 bytes");
        assert_eq!(format_size(1), "1 byte");
        assert_eq!(format_size(1023), "1.023 bytes");
        assert_eq!(format_size(1024), "1 KB");
        assert_eq!(format_size(1025), "2 KB");
        assert_eq!(format_size(500 * 1024), "500 KB");
        assert_eq!(format_size(3_000_000), "2,9 MB");
        assert_eq!(format_size(1_610_612_736), "1,5 GB");
        assert_eq!(format_size(5 << 40), "5,0 TB");
    }

    #[test]
    fn columns_have_spanish_titles_and_sort_keys() {
        let titles: Vec<_> = COLUMNS.iter().map(|c| c.title()).collect();
        assert_eq!(titles, ["Nombre", "Carpeta", "Tamaño", "Modificado"]);
        assert_eq!(Column::Folder.sort_key(), SortKey::Path);
    }
}
