//! Núcleo de Lince: índice compacto en memoria, consultas y búsqueda.

mod drive;
mod fold;
mod index;
mod mft;
mod query;
mod search;
mod time;
mod walk;
#[cfg(windows)]
mod windows;

pub use drive::DriveKind;
pub use fold::fold_into;
pub use index::{
    filetime_to_unix, Entry, Index, IndexBuilder, FLAG_DIR, FLAG_HIDDEN, FLAG_ROOT, FLAG_SYSTEM,
    NO_PARENT,
};
pub use mft::index_from_mft;
pub use query::{parse_query, Kind, Pattern, Query, QueryError};
pub use search::{search, sort_results, SortKey};
pub use time::{filetime_from_local, format_filetime_local};
pub use walk::{walk, WalkStats};
#[cfg(windows)]
pub use windows::{drive_kind, ntfs_volumes, Volume};
