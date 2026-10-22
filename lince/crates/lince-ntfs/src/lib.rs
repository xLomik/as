//! Lectura de volúmenes NTFS a partir de bytes crudos.
//!
//! Nada de este crate depende de Windows: trabaja sobre cualquier fuente que
//! implemente [`ReadAt`] (el volumen `\\.\C:` en Windows, o una imagen de disco
//! en las pruebas).

mod boot;
mod bytes;
mod error;
mod reader;
mod record;
mod runs;

#[cfg(test)]
mod testutil;

pub use boot::{parse_boot_sector, BootSector};
pub use error::NtfsError;
pub use reader::{read_layout, read_records, Extent, MftLayout, MftScan, ReadAt, ReadOptions};
pub use record::{
    apply_fixups, parse_attr_list, parse_record, AttrList, AttrListEntry, FileName,
    NonResidentAttr, ParsedRecord, StdInfo, ATTR_BITMAP, ATTR_DATA, NS_DOS, NS_POSIX, NS_WIN32,
    NS_WIN32_AND_DOS,
};
pub use runs::{decode_runs, DataRun};
