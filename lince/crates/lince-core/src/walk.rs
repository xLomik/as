//! Recorrido paralelo de carpetas: para unidades de red, FAT/exFAT o cuando
//! no hay permisos para leer la MFT. Más lento que la MFT, funciona en todo.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::SystemTime;

use crate::index::EPOCH_DIFF_SECS;
use crate::{Index, IndexBuilder, FLAG_DIR, FLAG_HIDDEN, FLAG_ROOT, FLAG_SYSTEM, NO_PARENT};

/// Contadores del recorrido.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WalkStats {
    pub dirs: u64,
    pub files: u64,
    /// Carpetas o entradas que no se pudieron leer (p. ej. sin permiso).
    pub errors: u64,
}

struct Raw {
    name: String,
    flags: u16,
    size: u64,
    modified: u64,
    created: u64,
    /// Id de la carpeta si es una que se va a recorrer.
    child: Option<u32>,
}

struct Listing {
    dir: u32,
    entries: Vec<Raw>,
}

struct Ctx {
    next_id: AtomicU32,
    dirs: AtomicU64,
    files: AtomicU64,
    errors: AtomicU64,
    listings: Mutex<Vec<Listing>>,
}

const ROOT_ID: u32 = 0;

/// Como [`walk`], con `threads` listados de carpeta simultáneos. En unidades
/// de red cada listado espera la respuesta del servidor, así que conviene
/// tener muchos más en vuelo que núcleos.
pub fn walk_with_threads(root: &Path, threads: usize) -> io::Result<(Index, WalkStats)> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads.max(1))
        .thread_name(|i| format!("lince-red-{i}"))
        .build()
        .map_err(io::Error::other)?;
    pool.install(|| walk(root))
}

/// Recorre `root` en paralelo (una tarea de rayon por carpeta) sin seguir
/// enlaces simbólicos ni *junctions*.
pub fn walk(root: &Path) -> io::Result<(Index, WalkStats)> {
    let meta = fs::metadata(root)?;
    if !meta.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{} no es una carpeta", root.display()),
        ));
    }
    // La raíz sí debe poder leerse: si no, es un error y no un conteo.
    fs::read_dir(root)?;

    let ctx = Ctx {
        next_id: AtomicU32::new(ROOT_ID + 1),
        dirs: AtomicU64::new(0),
        files: AtomicU64::new(0),
        errors: AtomicU64::new(0),
        listings: Mutex::new(Vec::new()),
    };
    rayon::scope(|s| visit(s, root.to_path_buf(), ROOT_ID, &ctx));

    let mut listings = ctx.listings.into_inner().unwrap();
    listings.sort_unstable_by_key(|l| l.dir);
    let total: usize = listings.iter().map(|l| l.entries.len()).sum();
    let ids = ctx.next_id.load(Ordering::Relaxed) as usize;

    let mut b = IndexBuilder::with_capacity(&label_for(root), std::path::MAIN_SEPARATOR, total + 1);
    let (modified, created) = times(&meta);
    let root_idx = b.push("", NO_PARENT, FLAG_DIR | FLAG_ROOT, 0, modified, created);
    let mut dir_entry = vec![NO_PARENT; ids];
    dir_entry[ROOT_ID as usize] = root_idx;
    let mut pending = Vec::with_capacity(total);
    for l in &listings {
        for r in &l.entries {
            let idx = b.push(&r.name, NO_PARENT, r.flags, r.size, r.modified, r.created);
            if let Some(child) = r.child {
                dir_entry[child as usize] = idx;
            }
            pending.push((idx, l.dir));
        }
    }
    for (idx, dir) in pending {
        b.set_parent(idx, dir_entry[dir as usize]);
    }
    let stats = WalkStats {
        dirs: ctx.dirs.load(Ordering::Relaxed),
        files: ctx.files.load(Ordering::Relaxed),
        errors: ctx.errors.load(Ordering::Relaxed),
    };
    Ok((b.finish(), stats))
}

fn visit<'s>(s: &rayon::Scope<'s>, path: PathBuf, dir: u32, ctx: &'s Ctx) {
    let rd = match fs::read_dir(&path) {
        Ok(rd) => rd,
        Err(_) => {
            ctx.errors.fetch_add(1, Ordering::Relaxed);
            return;
        }
    };
    let mut entries = Vec::new();
    for item in rd {
        let Ok(item) = item else {
            ctx.errors.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        // DirEntry::metadata no sigue enlaces (en Windows sale gratis del listado).
        let (Ok(ft), Ok(meta)) = (item.file_type(), item.metadata()) else {
            ctx.errors.fetch_add(1, Ordering::Relaxed);
            continue;
        };
        let name = item.file_name().to_string_lossy().into_owned();
        let descend = ft.is_dir() && !ft.is_symlink();
        let mut flags = attribute_flags(&name, &meta);
        let child = if descend {
            flags |= FLAG_DIR;
            ctx.dirs.fetch_add(1, Ordering::Relaxed);
            let id = ctx.next_id.fetch_add(1, Ordering::Relaxed);
            let sub = item.path();
            s.spawn(move |s| visit(s, sub, id, ctx));
            Some(id)
        } else {
            ctx.files.fetch_add(1, Ordering::Relaxed);
            None
        };
        let (modified, created) = times(&meta);
        entries.push(Raw {
            name,
            flags,
            size: if descend { 0 } else { meta.len() },
            modified,
            created,
            child,
        });
    }
    ctx.listings.lock().unwrap().push(Listing { dir, entries });
}

/// Ruta de la raíz tal como se mostrará al inicio de cada resultado.
fn label_for(root: &Path) -> String {
    let s = root.to_string_lossy();
    let trimmed = s.trim_end_matches(['/', '\\']);
    // "/" o "C:\" quedarían vacíos o sin barra.
    if trimmed.is_empty() || trimmed.ends_with(':') {
        s.into_owned()
    } else {
        trimmed.to_string()
    }
}

fn to_filetime(t: io::Result<SystemTime>) -> u64 {
    let Ok(t) = t else { return 0 };
    let (secs, nanos) = match t.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => (d.as_secs() as i64, d.subsec_nanos()),
        Err(e) => (-(e.duration().as_secs() as i64), 0),
    };
    match u64::try_from(secs + EPOCH_DIFF_SECS) {
        Ok(s) => s * 10_000_000 + u64::from(nanos / 100),
        Err(_) => 0,
    }
}

fn times(meta: &fs::Metadata) -> (u64, u64) {
    (to_filetime(meta.modified()), to_filetime(meta.created()))
}

#[cfg(windows)]
fn attribute_flags(_name: &str, meta: &fs::Metadata) -> u16 {
    use std::os::windows::fs::MetadataExt;
    let attrs = meta.file_attributes();
    let mut flags = 0;
    if attrs & 0x2 != 0 {
        flags |= FLAG_HIDDEN;
    }
    if attrs & 0x4 != 0 {
        flags |= FLAG_SYSTEM;
    }
    flags
}

#[cfg(not(windows))]
fn attribute_flags(name: &str, _meta: &fs::Metadata) -> u16 {
    let _ = FLAG_SYSTEM;
    if name.starts_with('.') {
        FLAG_HIDDEN
    } else {
        0
    }
}
