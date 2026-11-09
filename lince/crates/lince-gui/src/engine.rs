//! Motor en segundo plano: indexa y busca sin bloquear la ventana.
//!
//! La ventana manda trabajos por un canal y recibe eventos por `notify`
//! (en Windows, un `PostMessage`). Cada fuente se indexa en su propio hilo y
//! entra en las búsquedas en cuanto termina: un disco local está listo en
//! segundos aunque una carpeta de red tarde minutos. De varias búsquedas
//! pendientes solo se hace la última.
//!
//! Si se busca dentro de los archivos, primero se filtra por nombre (al
//! instante) y luego otro hilo lee los candidatos y manda resultados
//! parciales; una búsqueda nueva lo cancela.

use std::sync::{mpsc, Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lince_core::content::{content_candidates, content_threads, needle, ContentScan};
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
    /// Avance de la búsqueda dentro de los archivos (`None`: solo nombres).
    pub content: Option<ContentStatus>,
}

/// Cuánto lleva revisado la búsqueda dentro de los archivos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContentStatus {
    pub done: usize,
    pub total: usize,
    /// Archivos que no se pudieron leer (dañados, protegidos, sin permiso).
    pub unreadable: usize,
    pub finished: bool,
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
    /// Texto a buscar dentro de los archivos (caja aparte de la ventana).
    content: String,
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
    /// Avance de una búsqueda dentro de los archivos.
    Content {
        generation: u64,
        indexes: Indexes,
        hits: Vec<Hit>,
        status: ContentStatus,
        started: Instant,
        key: SortKey,
        desc: bool,
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
        self.search_content(generation, query, "", key, desc, volume);
    }

    /// Como [`Engine::search`], y además los archivos deben contener
    /// `content` (vacío = no se mira el contenido).
    pub fn search_content(
        &self,
        generation: u64,
        query: &str,
        content: &str,
        key: SortKey,
        desc: bool,
        volume: Option<String>,
    ) {
        let _ = self.tx.send(Job::Search(SearchJob {
            generation,
            query: query.to_string(),
            content: content.to_string(),
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
    // Búsqueda en contenido en marcha y su número de búsqueda.
    let mut scan: Option<(u64, Arc<ContentScan>)> = None;
    while let Ok(first) = rx.recv() {
        let mut newest = None;
        for job in std::iter::once(first).chain(rx.try_iter()) {
            match job {
                Job::Search(s) => newest = Some(s),
                Job::Reindex(sources) => {
                    if let Some((_, old)) = scan.take() {
                        old.cancel();
                    }
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
                Job::Content {
                    generation,
                    indexes,
                    mut hits,
                    status,
                    started,
                    key,
                    desc,
                } => {
                    // De una búsqueda ya reemplazada: no se muestra.
                    if scan.as_ref().is_none_or(|(g, _)| *g != generation) {
                        continue;
                    }
                    sort_hits(&indexes, &mut hits, key, desc);
                    notify(Event::Results(Results {
                        generation,
                        indexes,
                        hits,
                        elapsed: started.elapsed(),
                        error: None,
                        content: Some(status),
                    }));
                }
            }
        }
        if let Some(s) = newest {
            if let Some((_, old)) = scan.take() {
                old.cancel();
            }
            // Sin ningún índice listo todavía, la búsqueda espera.
            let waiting = indexes.is_empty() && batch.as_ref().is_none_or(|b| b.pending > 0);
            if !waiting {
                let started = Instant::now();
                let (results, needles) = run_search(&indexes, &s);
                if needles.is_empty() || results.error.is_some() {
                    notify(Event::Results(results));
                } else {
                    let candidates = content_candidates(&indexes, &results.hits);
                    notify(Event::Results(Results {
                        hits: Vec::new(),
                        content: Some(ContentStatus {
                            done: 0,
                            total: candidates.len(),
                            unreadable: 0,
                            finished: false,
                        }),
                        ..results
                    }));
                    let new = Arc::new(ContentScan::new());
                    scan = Some((s.generation, Arc::clone(&new)));
                    spawn_content(ContentJob {
                        generation: s.generation,
                        indexes: Arc::clone(&indexes),
                        candidates,
                        needles,
                        scan: new,
                        started,
                        key: s.key,
                        desc: s.desc,
                        tx: tx.clone(),
                    });
                }
            }
            last = Some(s);
        }
    }
}

/// Lo que necesita el hilo que revisa el contenido.
struct ContentJob {
    generation: u64,
    indexes: Indexes,
    candidates: Vec<Hit>,
    needles: Vec<Vec<u8>>,
    scan: Arc<ContentScan>,
    started: Instant,
    key: SortKey,
    desc: bool,
    tx: mpsc::Sender<Job>,
}

/// Cada cuánto se mandan resultados parciales.
const CONTENT_TICK: Duration = Duration::from_millis(250);

/// Revisa el contenido en otro hilo y manda el avance al motor. Cancelado,
/// no manda nada más.
fn spawn_content(job: ContentJob) {
    let _ = thread::Builder::new()
        .name("lince-contenido".into())
        .spawn(move || {
            let total = job.candidates.len();
            let send = |finished: bool| {
                let _ = job.tx.send(Job::Content {
                    generation: job.generation,
                    indexes: Arc::clone(&job.indexes),
                    hits: job.scan.found(),
                    status: ContentStatus {
                        done: job.scan.done(),
                        total,
                        unreadable: job.scan.unreadable(),
                        finished,
                    },
                    started: job.started,
                    key: job.key,
                    desc: job.desc,
                });
            };
            thread::scope(|scope| {
                let runner = scope.spawn(|| {
                    job.scan.run(
                        &job.indexes,
                        &job.candidates,
                        &job.needles,
                        content_threads(),
                    )
                });
                while !runner.is_finished() && !job.scan.is_cancelled() {
                    thread::sleep(CONTENT_TICK);
                    if !runner.is_finished() && !job.scan.is_cancelled() {
                        send(false);
                    }
                }
            });
            if !job.scan.is_cancelled() {
                send(true);
            }
        });
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

/// Búsqueda por nombre; devuelve además los textos a buscar dentro de los
/// archivos (de la consulta y de la caja de contenido).
fn run_search(indexes: &Indexes, s: &SearchJob) -> (Results, Vec<Vec<u8>>) {
    let start = Instant::now();
    let mut needles = Vec::new();
    let (hits, error) = match parse_query(&s.query) {
        Err(e) => (Vec::new(), Some(e.to_string())),
        Ok(q) => {
            needles = q.content.clone();
            let extra = needle(&s.content);
            if !extra.is_empty() {
                needles.push(extra);
            }
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
    let results = Results {
        generation: s.generation,
        indexes: Arc::clone(indexes),
        hits,
        elapsed: start.elapsed(),
        error,
        content: None,
    };
    (results, needles)
}
