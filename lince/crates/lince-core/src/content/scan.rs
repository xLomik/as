//! Revisa el contenido de muchos archivos a la vez, con su propio grupo de
//! hilos (la búsqueda por nombre nunca espera por él), y se puede cancelar
//! a mitad.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use rayon::prelude::*;

use super::{file_contains, format_of};
use crate::{Hit, Index, FLAG_DIR};

/// Archivos de `hits` con contenido legible, del más pequeño al más grande:
/// así los primeros resultados aparecen enseguida.
pub fn content_candidates<I: AsRef<Index>>(indexes: &[I], hits: &[Hit]) -> Vec<Hit> {
    let mut out: Vec<Hit> = hits
        .iter()
        .copied()
        .filter(|&(s, i)| {
            let ix = indexes[s].as_ref();
            ix.entry(i).flags & FLAG_DIR == 0 && format_of(Path::new(ix.name(i))).is_some()
        })
        .collect();
    out.sort_by_key(|&(s, i)| indexes[s].as_ref().entry(i).size);
    out
}

/// Una búsqueda en contenido en marcha. Se consulta desde otro hilo mientras
/// avanza.
#[derive(Default)]
pub struct ContentScan {
    cancel: AtomicBool,
    done: AtomicUsize,
    unreadable: AtomicUsize,
    found: Mutex<Vec<Hit>>,
}

impl ContentScan {
    pub fn new() -> ContentScan {
        ContentScan::default()
    }

    /// Los archivos que falten no se revisan; el que se esté leyendo termina.
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// Archivos ya revisados.
    pub fn done(&self) -> usize {
        self.done.load(Ordering::Relaxed)
    }

    /// Archivos que no se pudieron leer (dañados, protegidos, sin permiso).
    pub fn unreadable(&self) -> usize {
        self.unreadable.load(Ordering::Relaxed)
    }

    /// Lo encontrado hasta ahora.
    pub fn found(&self) -> Vec<Hit> {
        self.found.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Revisa `candidates` con `threads` hilos. Al volver, [`found`] tiene
    /// los que contienen todos los textos (en cualquier orden).
    ///
    /// [`found`]: ContentScan::found
    pub fn run<I: AsRef<Index> + Sync>(
        &self,
        indexes: &[I],
        candidates: &[Hit],
        needles: &[Vec<u8>],
        threads: usize,
    ) {
        let check = |&(s, i): &Hit| {
            if self.is_cancelled() {
                return;
            }
            let path = indexes[s].as_ref().path(i);
            match file_contains(Path::new(&path), needles) {
                Ok(true) => self
                    .found
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .push((s, i)),
                Ok(false) => {}
                Err(_) => {
                    self.unreadable.fetch_add(1, Ordering::Relaxed);
                }
            }
            self.done.fetch_add(1, Ordering::Relaxed);
        };
        match rayon::ThreadPoolBuilder::new()
            .num_threads(threads.max(1))
            .thread_name(|n| format!("lince-contenido-{n}"))
            .build()
        {
            // Uno por uno: lo que sale antes de cada archivo es lo pequeño.
            Ok(pool) => pool.install(|| candidates.par_iter().with_max_len(1).for_each(check)),
            Err(_) => candidates.iter().for_each(check),
        }
    }
}

/// Hilos para leer contenido: uno por núcleo (extraer texto de un PDF o un
/// Excel usa CPU; con menos de 4, la espera del disco domina).
pub fn content_threads() -> usize {
    std::thread::available_parallelism().map_or(4, |n| n.get().max(4))
}
