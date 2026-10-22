//! Ajuste de la lectura según el tipo de disco.

use lince_ntfs::ReadOptions;

const MIB: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveKind {
    Nvme,
    Ssd,
    Hdd,
    Unknown,
}

impl DriveKind {
    /// NVMe aguanta colas muy profundas; en un HDD varias lecturas a la vez
    /// solo añaden saltos del cabezal.
    pub fn read_options(self) -> ReadOptions {
        let (threads, chunk_mib) = match self {
            DriveKind::Nvme => (32, 4),
            DriveKind::Ssd => (8, 2),
            DriveKind::Hdd => (2, 1),
            DriveKind::Unknown => (4, 2),
        };
        ReadOptions {
            threads,
            chunk_bytes: chunk_mib * MIB,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            DriveKind::Nvme => "NVMe",
            DriveKind::Ssd => "SSD",
            DriveKind::Hdd => "disco mecánico",
            DriveKind::Unknown => "desconocido",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_match_the_spec() {
        let p = |k: DriveKind| {
            let o = k.read_options();
            (o.threads, o.chunk_bytes / MIB)
        };
        assert_eq!(p(DriveKind::Nvme), (32, 4));
        assert_eq!(p(DriveKind::Ssd), (8, 2));
        assert_eq!(p(DriveKind::Hdd), (2, 1));
        assert_eq!(p(DriveKind::Unknown), (4, 2));
    }
}
