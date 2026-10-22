//! Conversión de los registros de la MFT al índice.

use lince_ntfs::{FileName, MftScan};

use crate::{Index, IndexBuilder, FLAG_DIR, FLAG_HIDDEN, FLAG_ROOT, FLAG_SYSTEM, NO_PARENT};

/// Registro de la carpeta raíz de todo volumen NTFS.
const ROOT_FRS: u64 = 5;
/// Los registros 0–15 son metadatos (`$MFT`, `$Bitmap`, `$Extend`...).
const FIRST_USER_FRS: u64 = 16;

const ATTR_HIDDEN: u32 = 0x2;
const ATTR_SYSTEM: u32 = 0x4;

#[derive(Clone, Copy, Default)]
struct Meta {
    present: bool,
    is_dir: bool,
    sequence: u16,
    size: Option<u64>,
    modified: u64,
    created: u64,
    attributes: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Unknown,
    Visiting,
    Keep,
    Excluded,
}

fn is_metadata(frs: u64) -> bool {
    frs < FIRST_USER_FRS && frs != ROOT_FRS
}

/// Construye el índice de un volumen (`label` = `C:`). Hay una entrada por
/// nombre: un archivo con varios hard links aparece en cada una de sus rutas.
/// Se omiten los metadatos de NTFS y todo lo que cuelga de ellos (`$Extend`).
pub fn index_from_mft(label: &str, scan: MftScan) -> Index {
    let records = scan.records;
    let len = records
        .last()
        .map_or(0, |r| r.frs as usize + 1)
        .max(ROOT_FRS as usize + 1);

    let mut meta = vec![Meta::default(); len];
    for r in records.iter().filter(|r| r.base_frs.is_none()) {
        let std = r.std_info.unwrap_or(lince_ntfs::StdInfo {
            created: 0,
            modified: 0,
            accessed: 0,
            attributes: 0,
        });
        meta[r.frs as usize] = Meta {
            present: true,
            is_dir: r.is_dir,
            sequence: r.sequence,
            size: r.data_size,
            modified: std.modified,
            created: std.created,
            attributes: std.attributes,
        };
    }
    // En archivos con lista de atributos el $DATA puede estar en una extensión.
    for r in &records {
        if let (Some(base), Some(size)) = (r.base_frs, r.data_size) {
            if let Some(m) = meta.get_mut(base as usize).filter(|m| m.present) {
                m.size.get_or_insert(size);
            }
        }
    }

    // Nombres de todos los registros, asignados a su registro base.
    let names: Vec<(u64, &FileName)> = records
        .iter()
        .flat_map(|r| {
            let owner = r.base_frs.unwrap_or(r.frs);
            r.names.iter().map(move |n| (owner, n))
        })
        .filter(|&(owner, _)| meta.get(owner as usize).is_some_and(|m| m.present))
        .collect();

    // Padre de cada carpeta (para saber qué cuelga de los metadatos).
    let mut dir_parent = vec![u64::MAX; len];
    for &(owner, n) in &names {
        if meta[owner as usize].is_dir && dir_parent[owner as usize] == u64::MAX {
            dir_parent[owner as usize] = n.parent_frs;
        }
    }
    let mut state = vec![State::Unknown; len];
    let mut excluded = |frs: u64| resolve(frs, &dir_parent, &mut state) == State::Excluded;

    let mut b = IndexBuilder::with_capacity(label, '\\', names.len() + 1);
    let root_meta = meta[ROOT_FRS as usize];
    let root = b.push(
        "",
        NO_PARENT,
        FLAG_DIR | FLAG_ROOT,
        0,
        root_meta.modified,
        root_meta.created,
    );
    let mut dir_entry = vec![NO_PARENT; len];
    dir_entry[ROOT_FRS as usize] = root;

    let mut pending = Vec::with_capacity(names.len());
    for &(owner, n) in &names {
        if owner == ROOT_FRS || is_metadata(owner) || excluded(n.parent_frs) {
            continue;
        }
        let m = meta[owner as usize];
        let mut flags = 0;
        if m.is_dir {
            flags |= FLAG_DIR;
        }
        if m.attributes & ATTR_HIDDEN != 0 {
            flags |= FLAG_HIDDEN;
        }
        if m.attributes & ATTR_SYSTEM != 0 {
            flags |= FLAG_SYSTEM;
        }
        let size = if m.is_dir { 0 } else { m.size.unwrap_or(0) };
        let idx = b.push(&n.name, NO_PARENT, flags, size, m.modified, m.created);
        if m.is_dir && dir_entry[owner as usize] == NO_PARENT {
            dir_entry[owner as usize] = idx;
        }
        pending.push((idx, n.parent_frs, n.parent_seq));
    }
    for (idx, parent_frs, parent_seq) in pending {
        let Some(pm) = meta.get(parent_frs as usize) else {
            continue;
        };
        // Un número de secuencia distinto = el padre se borró y su registro se reutilizó.
        let same = parent_seq == 0 || pm.sequence == parent_seq;
        let parent = dir_entry[parent_frs as usize];
        if pm.present && pm.is_dir && same && parent != NO_PARENT {
            b.set_parent(idx, parent);
        }
    }
    b.finish()
}

/// ¿La carpeta `frs` es metadato o cuelga de uno? Iterativo para no
/// desbordar la pila con árboles profundos; los ciclos se tratan como normales.
fn resolve(frs: u64, dir_parent: &[u64], state: &mut [State]) -> State {
    let mut chain = Vec::new();
    let mut cur = frs;
    let result = loop {
        let Some(&s) = state.get(cur as usize) else {
            break State::Keep; // padre fuera de la MFT: huérfano, no metadato
        };
        match s {
            State::Keep | State::Excluded => break s,
            State::Visiting => break State::Keep,
            State::Unknown => {}
        }
        if cur == ROOT_FRS {
            break State::Keep;
        }
        if is_metadata(cur) {
            break State::Excluded;
        }
        state[cur as usize] = State::Visiting;
        chain.push(cur);
        match dir_parent[cur as usize] {
            u64::MAX => break State::Keep,
            p => cur = p,
        }
    };
    for f in chain {
        state[f as usize] = result;
    }
    if (frs as usize) < state.len() && state[frs as usize] == State::Unknown {
        state[frs as usize] = result;
    }
    result
}
