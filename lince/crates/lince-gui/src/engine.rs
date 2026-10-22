//! Motor en segundo plano: indexa y busca sin bloquear la ventana.
//!
//! La ventana manda trabajos por un canal y recibe eventos por `notify`
//! (en Windows, un `PostMessage`). De varias búsquedas pendientes solo se
//! hace la última: mientras se escribe rápido no se acumula trabajo viejo.

use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lince_core::{parse_query, search, sort_hits, BuildStats, Hit, Index, SortKey, Source};
use rayon::prelude::*;

/// Resultado de una búsqueda. Lleva los índices con los que se calculó, así
/// la lista se pinta bien aunque mientras tanto se reindexe.
pub struct Results {
    pub generation: u64,
    pub indexes: Arc<Vec<Index>>,
    pub hits: Vec<Hit>,
    pub elapsed: Duration,
    pub error: Option<String>,
}

pub enum Event {
    /// Empezó la indexación de estas fuentes.
    Indexing {
        labels: Vec<String>,
    },
    Indexed {
        entries: usize,
        elapsed: Duration,
        stats: Vec<BuildStats>,
        /// Avisos y errores para el usuario (p. ej. sin administrador).
        warnings: Vec<String>,
    },
    Results(Results),
}

struct SearchJob {
    generation: u64,
    query: String,
    key: SortKey,
    desc: bool,
    volume: Option<String>,
}

enum Job {
    Reindex(Vec<Source>),
    Search(SearchJob),
}

pub struct Engine {
    tx: mpsc::Sender<Job>,
}

impl Engine {
    pub fn start(notify: impl Fn(Event) + Send + 'static) -> Engine {
        let (tx, rx) = mpsc::channel();
        thread::Builder::new()
            .name("lince-motor".into())
            .spawn(move || run(rx, notify))
            .expect("no se pudo crear el hilo del motor");
        Engine { tx }
    }

    pub fn reindex(&self, sources: Vec<Source>) {
        let _ = self.tx.send(Job::Reindex(sources));
    }

    /// `volume`: solo esa fuente (`C:`); `None` = todas.
    pub fn search(
        &self,
        generation: u64,
        query: &str,
        key: SortKey,
        desc: bool,
        volume: Option<String>,
    ) {
        let _ = self.tx.send(Job::Search(SearchJob {
            generation,
            query: query.to_string(),
            key,
            desc,
            volume,
        }));
    }
}

fn run(rx: mpsc::Receiver<Job>, notify: impl Fn(Event)) {
    let mut indexes: Arc<Vec<Index>> = Arc::new(Vec::new());
    let mut indexed = false;
    let mut last: Option<SearchJob> = None;
    while let Ok(first) = rx.recv() {
        let mut reindex = None;
        let mut newest = None;
        for job in std::iter::once(first).chain(rx.try_iter()) {
            match job {
                Job::Reindex(sources) => reindex = Some(sources),
                Job::Search(s) => newest = Some(s),
            }
        }
        if let Some(sources) = reindex {
            indexes = Arc::new(build(sources, &notify));
            indexed = true;
            newest = newest.or(last.take());
        }
        if let Some(s) = newest {
            if indexed {
                notify(Event::Results(run_search(&indexes, &s)));
            }
            last = Some(s);
        }
    }
}

fn build(sources: Vec<Source>, notify: &impl Fn(Event)) -> Vec<Index> {
    notify(Event::Indexing {
        labels: sources.iter().map(Source::label).collect(),
    });
    let start = Instant::now();
    let warnings = Mutex::new(Vec::new());
    let built: Vec<_> = sources
        .par_iter()
        .map(|s| s.build(&|w| warnings.lock().unwrap().push(w)))
        .collect();
    let mut warnings = warnings.into_inner().unwrap();
    let mut indexes = Vec::new();
    let mut stats = Vec::new();
    for b in built {
        match b {
            Ok(b) => {
                stats.push(b.stats);
                indexes.push(b.index);
            }
            Err(e) => warnings.push(e),
        }
    }
    notify(Event::Indexed {
        entries: stats.iter().map(|s| s.entries).sum(),
        elapsed: start.elapsed(),
        stats,
        warnings,
    });
    indexes
}

fn run_search(indexes: &Arc<Vec<Index>>, s: &SearchJob) -> Results {
    let start = Instant::now();
    let (hits, error) = match parse_query(&s.query) {
        Err(e) => (Vec::new(), Some(e.to_string())),
        Ok(q) => {
            let mut hits: Vec<Hit> = indexes
                .iter()
                .enumerate()
                .filter(|(_, ix)| {
                    s.volume
                        .as_deref()
                        .is_none_or(|v| ix.label().eq_ignore_ascii_case(v))
                })
                .flat_map(|(n, ix)| search(ix, &q).into_iter().map(move |i| (n, i)))
                .collect();
            sort_hits(indexes, &mut hits, s.key, s.desc);
            (hits, None)
        }
    };
    Results {
        generation: s.generation,
        indexes: Arc::clone(indexes),
        hits,
        elapsed: start.elapsed(),
        error,
    }
}
