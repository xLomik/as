//! Búsqueda paralela sobre el índice.

use memchr::memmem;
use rayon::prelude::*;

use crate::query::{Kind, Query};
use crate::{fold_into, Index, FLAG_DIR, FLAG_ROOT};

/// Trozos por hilo: suficientes para repartir bien la carga.
const PARTS_PER_THREAD: usize = 8;

/// Devuelve las entradas que cumplen la consulta, en orden del índice.
///
/// Si hay un literal, se busca con SIMD sobre el buffer contiguo de nombres en
/// minúsculas, partido en trozos que se recorren en paralelo; cada coincidencia
/// se comprueba después con el resto de condiciones.
pub fn search(ix: &Index, q: &Query) -> Vec<u32> {
    let literal = q
        .terms
        .iter()
        .map(|t| t.longest_literal())
        .chain(
            q.exts
                .iter()
                .filter(|_| q.exts.len() == 1)
                .map(|e| e.as_slice()),
        )
        .max_by_key(|l| l.len())
        .filter(|l| !l.is_empty());
    match literal {
        Some(needle) => search_literal(ix, q, needle),
        None => (0..ix.len() as u32)
            .into_par_iter()
            .filter(|&i| matches(ix, q, i))
            .collect(),
    }
}

fn search_literal(ix: &Index, q: &Query, needle: &[u8]) -> Vec<u32> {
    let n = ix.len();
    let parts = (rayon::current_num_threads() * PARTS_PER_THREAD).max(1);
    let per = n.div_ceil(parts).max(1);
    let finder = memmem::Finder::new(needle);
    let folded = ix.folded();
    let chunks: Vec<Vec<u32>> = (0..n.div_ceil(per))
        .into_par_iter()
        .map(|c| {
            let (a, b) = (c * per, ((c + 1) * per).min(n));
            let (lo, hi) = (ix.folded_start(a), ix.folded_start(b));
            let mut out = Vec::new();
            let mut pos = lo;
            while let Some(m) = finder.find(&folded[pos..hi]) {
                let e = ix.entry_at_folded(pos + m);
                if matches(ix, q, e) {
                    out.push(e);
                }
                // Siguiente nombre: cada entrada aparece una sola vez.
                pos = ix.folded_start(e as usize + 1);
            }
            out
        })
        .collect();
    chunks.concat()
}

fn matches(ix: &Index, q: &Query, i: u32) -> bool {
    let e = ix.entry(i);
    if e.flags & FLAG_ROOT != 0 {
        return false;
    }
    let is_dir = e.flags & FLAG_DIR != 0;
    match q.kind {
        Kind::Files if is_dir => return false,
        Kind::Folders if !is_dir => return false,
        _ => {}
    }
    let name = ix.folded_name(i);
    if !q.terms.iter().all(|t| t.matches(name)) || q.not_terms.iter().any(|t| t.matches(name)) {
        return false;
    }
    if !q.exts.is_empty() {
        let ext = match name.iter().rposition(|&b| b == b'.') {
            Some(dot) => &name[dot + 1..],
            None => return false,
        };
        if !q.exts.iter().any(|x| x == ext) {
            return false;
        }
    }
    if let Some((lo, hi)) = q.size {
        if is_dir || e.size < lo || e.size > hi {
            return false;
        }
    }
    let in_range = |t: u64, r: Option<(u64, u64)>| r.is_none_or(|(lo, hi)| t >= lo && t < hi);
    if !in_range(e.modified, q.modified) || !in_range(e.created, q.created) {
        return false;
    }
    if let Some(re) = &q.regex {
        if !re.is_match(ix.name(i)) {
            return false;
        }
    }
    if !q.path_terms.is_empty() {
        let mut path = Vec::new();
        fold_into(&ix.path(i), &mut path);
        if !q
            .path_terms
            .iter()
            .all(|t| memmem::find(&path, t).is_some())
        {
            return false;
        }
    }
    true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Path,
    Size,
    Modified,
}

/// Ordena resultados en paralelo. Empates: por nombre y luego por posición.
pub fn sort_results(ix: &Index, res: &mut [u32], key: SortKey, desc: bool) {
    let by_name = |a: &u32, b: &u32| ix.folded_name(*a).cmp(ix.folded_name(*b)).then(a.cmp(b));
    match key {
        SortKey::Name => res.par_sort_by(by_name),
        SortKey::Size => res.par_sort_by(|a, b| {
            ix.entry(*a)
                .size
                .cmp(&ix.entry(*b).size)
                .then_with(|| by_name(a, b))
        }),
        SortKey::Modified => res.par_sort_by(|a, b| {
            ix.entry(*a)
                .modified
                .cmp(&ix.entry(*b).modified)
                .then_with(|| by_name(a, b))
        }),
        SortKey::Path => {
            // Componentes en minúsculas unidos por \0: compara carpeta por carpeta.
            let mut keyed: Vec<(Vec<u8>, u32)> = res
                .par_iter()
                .map(|&i| {
                    let mut k = Vec::new();
                    for c in ix.components(i) {
                        fold_into(c, &mut k);
                        k.push(0);
                    }
                    (k, i)
                })
                .collect();
            keyed.par_sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
            for (slot, (_, i)) in res.iter_mut().zip(keyed) {
                *slot = i;
            }
        }
    }
    if desc {
        res.reverse();
    }
}
