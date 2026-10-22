use std::fmt;
use std::io;

/// Errores al interpretar estructuras NTFS.
#[derive(Debug)]
pub enum NtfsError {
    /// El sector de arranque no es de un volumen NTFS.
    NotNtfs,
    /// El sector de arranque es NTFS pero tiene valores imposibles.
    BadBootSector(&'static str),
    /// Un registro FILE está corrupto o mal formado.
    BadRecord(&'static str),
    /// La lista de *data runs* está mal formada.
    BadRuns,
    /// No se sabe dónde está en disco el registro pedido de la MFT.
    MftUnmapped(u64),
    Io(io::Error),
}

impl fmt::Display for NtfsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NtfsError::NotNtfs => write!(f, "el volumen no es NTFS"),
            NtfsError::BadBootSector(why) => write!(f, "sector de arranque NTFS inválido: {why}"),
            NtfsError::BadRecord(why) => write!(f, "registro de la MFT inválido: {why}"),
            NtfsError::BadRuns => write!(f, "lista de data runs inválida"),
            NtfsError::MftUnmapped(frs) => {
                write!(f, "no se encontró en disco el registro {frs} de la MFT")
            }
            NtfsError::Io(e) => write!(f, "error de lectura: {e}"),
        }
    }
}

impl std::error::Error for NtfsError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            NtfsError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for NtfsError {
    fn from(e: io::Error) -> Self {
        NtfsError::Io(e)
    }
}
