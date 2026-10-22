//! Núcleo de Lince: índice compacto en memoria, consultas y búsqueda.

mod fold;
mod index;
mod mft;

pub use fold::fold_into;
pub use index::{
    filetime_to_unix, Entry, Index, IndexBuilder, FLAG_DIR, FLAG_HIDDEN, FLAG_ROOT, FLAG_SYSTEM,
    NO_PARENT,
};
pub use mft::index_from_mft;
