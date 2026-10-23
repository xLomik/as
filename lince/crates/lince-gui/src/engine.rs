//! Motor en segundo plano: indexa y busca sin bloquear la ventana.
//!
//! La ventana manda trabajos por un canal y recibe eventos por `notify`
//! (en Windows, un `PostMessage`). Cada fuente se indexa en su propio hilo y
//! entra en las búsquedas en cuanto termina: un disco local está listo en
//! segundos aunque una carpeta de red tarde minutos. De varias búsquedas
//! pendientes solo se hace la última.

use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lince_core::{parse_query, search, sort_hits, BuildStats, Built, Hit, Index, SortKey, Source};

/// Índices con los que se calculó un resultado.
pub type Indexes = Arc<Vec<Arc<Index>>>;

/// Resultado de una búsqueda. Lleva los índices con los que se calculó, así
/// la lista se pinta bien aunque mientras tanto se reindexe.
pub struct Results {
    pub generation: u64,
    pub indexes: Indexes,
    pub hits: Vec<Hit>,
    pub elapsed: Duration,
    pub error: Option<String>,
}

pub enum Event {
    /// Empezó la indexación de estas fuentes.
    Indexing {
        labels: Vec<String>,
    },
    /// Terminó una fuente más (`pending` = las que faltan).
    Indexed {
        entries: usize,
        elapsed: Duration,
        stats: Vec<BuildStats>,
        /// Avisos y errores para el usuario (p. ej. sin administrador).
        warnings: Vec<String>,
        pending: usize,
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
    /// Una fuente terminó (de la tanda `epoch`).
    Built {
        epoch: u64,
        result: Result<Built, String>,
        warnings: Vec<String>,
    },
}

pub struct Engine {
    tx: mpsc::Sender<Job>,
}

impl Engine {
    pub fn start(notify: impl Fn(Event) + Send + 'static) -> Engine {
        let (tx, rx) = mpsc::channel();
        let worker_tx = tx.clone();
        thread::Builder::new()
            .name("lince-motor".into())
            .spawn(move || run(worker_tx, rx, notify))
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

/// Estado de la indexación en curso.
struct Batch {
    epoch: u64,
    started: Instant,
    pending: usize,
    stats: Vec<BuildStats>,
    warnings: Vec<String>,
}

fn run(tx: mpsc::Sender<Job>, rx: mpsc::Receiver<Job>, notify: impl Fn(Event)) {
    let mut indexes: Indexes = Arc::new(Vec::new());
    let mut batch: Option<Batch> = None;
    let mut last: Option<SearchJob> = None;
    while let Ok(first) = rx.recv() {
        let mut newest = None;
        for job in std::iter::once(first).chain(rx.try_iter()) {
            match job {
                Job::Search(s) => newest = Some(s),
                Job::Reindex(sources) => {
                    let epoch = batch.as_ref().map_or(1, |b| b.epoch + 1);
                    notify(Event::Indexing {
                        labels: sources.iter().map(Source::label).collect(),
                    });
                    indexes = Arc::new(Vec::new());
                    batch = Some(Batch {
                        epoch,
                        started: Instant::now(),
                        pending: sources.len(),
                        stats: Vec::new(),
                        warnings: Vec::new(),
                    });
                    for source in sources {
                        spawn_build(source, epoch, tx.clone());
                    }
                }
                Job::Built {
                    epoch,
                    result,
                    warnings,
                } => {
                    let Some(b) = batch.as_mut().filter(|b| b.epoch == epoch) else {
                        continue; // de una indexación anterior (F5)
                    };
                    b.pending -= 1;
                    b.warnings.extend(warnings);
                    match result {
                        Ok(built) => {
                            b.stats.push(built.stats);
                            let mut next: Vec<Arc<Index>> = indexes.iter().cloned().collect();
                            next.push(Arc::new(built.index));
                            indexes = Arc::new(next);
                        }
                        Err(e) => b.warnings.push(e),
                    }
                    notify(Event::Indexed {
                        entries: b.stats.iter().map(|s| s.entries).sum(),
                        elapsed: b.started.elapsed(),
                        stats: b.stats.clone(),
                        warnings: b.warnings.clone(),
                        pending: b.pending,
                    });
                    // Lo nuevo entra en la búsqueda que el usuario tiene en pantalla.
                    newest = newest.or(last.take());
                }
            }
        }
        if let Some(s) = newest {
            // Sin ningún índice listo todavía, la búsqueda espera.
            let waiting = indexes.is_empty() && batch.as_ref().is_none_or(|b| b.pending > 0);
            if !waiting {
                notify(Event::Results(run_search(&indexes, &s)));
            }
            last = Some(s);
        }
    }
}

/// Indexa `source` en un hilo propio. Siempre llega un `Job::Built`, aunque
/// el hilo no se pueda crear o la indexación falle de forma inesperada: si
/// no, la cuenta de pendientes nunca llegaría a cero.
fn spawn_build(source: Source, epoch: u64, tx: mpsc::Sender<Job>) {
    let label = source.label();
    let fail_tx = tx.clone();
    let spawned = thread::Builder::new()
        .name(format!("lince-indexa-{label}"))
        .spawn(move || {
            let label = source.label();
            let warnings = Mutex::new(Vec::new());
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                source.build(&|w| warnings.lock().unwrap().push(w))
            }))
            .unwrap_or_else(|_| Err(format!("{label}: error interno al indexar")));
            let warnings = warnings.into_inner().unwrap_or_default();
            let _ = tx.send(Job::Built {
                epoch,
                result,
                warnings,
            });
        });
    if let Err(e) = spawned {
        let _ = fail_tx.send(Job::Built {
            epoch,
            result: Err(format!("{label}: no se pudo crear un hilo: {e}")),
            warnings: Vec::new(),
        });
    }
}

fn run_search(indexes: &Indexes, s: &SearchJob) -> Results {
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
