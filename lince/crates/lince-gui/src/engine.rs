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

/// Quién pidió la búsqueda. Cada uno tiene su última búsqueda y su lectura
/// de contenido: la barra rápida no cancela lo que hace la ventana.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Client {
    Window,
    Bar,
}

impl Client {
    const ALL: usize = 2;

    fn slot(self) -> usize {
        self as usize
    }
}

/// Resultado de una búsqueda. Lleva los índices con los que se calculó, así
/// la lista se pinta bien aunque mientras tanto se reindexe.
pub struct Results {
    pub client: Client,
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
    client: Client,
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
        /// Nombre de la fuente pedida (el del índice puede variar: `C:` se
        /// recorre como `C:\`).
        source: String,
        result: Result<Built, String>,
        warnings: Vec<String>,
    },
    /// Avance de una búsqueda dentro de los archivos.
    Content {
        client: Client,
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
        self.search_from(
            Client::Window,
            generation,
            query,
            content,
            key,
            desc,
            volume,
        );
    }

    /// Búsqueda de `client`; sus resultados llevan el mismo `client`.
    #[allow(clippy::too_many_arguments)]
    pub fn search_from(
        &self,
        client: Client,
        generation: u64,
        query: &str,
        content: &str,
        key: SortKey,
        desc: bool,
        volume: Option<String>,
    ) {
        let _ = self.tx.send(Job::Search(SearchJob {
            client,
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
    /// Fuentes pedidas: al terminar, los índices de otras se descartan.
    labels: Vec<String>,
}

fn run(tx: mpsc::Sender<Job>, rx: mpsc::Receiver<Job>, notify: impl Fn(Event)) {
    let mut indexes: Indexes = Arc::new(Vec::new());
    // De qué fuente es cada índice (mismo orden que `indexes`).
    let mut owners: Vec<String> = Vec::new();
    let mut batch: Option<Batch> = None;
    let mut last: [Option<SearchJob>; Client::ALL] = Default::default();
    // Búsqueda en contenido en marcha de cada cliente, con su número.
    let mut scans: [Option<(u64, Arc<ContentScan>)>; Client::ALL] = Default::default();
    let cancel = |scan: &mut Option<(u64, Arc<ContentScan>)>| {
        if let Some((_, old)) = scan.take() {
            old.cancel();
        }
    };
    while let Ok(first) = rx.recv() {
        let mut newest: [Option<SearchJob>; Client::ALL] = Default::default();
        for job in std::iter::once(first).chain(rx.try_iter()) {
            match job {
                Job::Search(s) => {
                    let k = s.client.slot();
                    newest[k] = Some(s);
                }
                // Los índices actuales siguen respondiendo hasta que llega el
                // nuevo de cada fuente: volver a leer nunca deja la búsqueda
                // vacía.
                Job::Reindex(sources) => {
                    scans.iter_mut().for_each(cancel);
                    let epoch = batch.as_ref().map_or(1, |b| b.epoch + 1);
                    let labels: Vec<String> = sources.iter().map(Source::label).collect();
                    notify(Event::Indexing {
                        labels: labels.clone(),
                    });
                    batch = Some(Batch {
                        epoch,
                        started: Instant::now(),
                        pending: sources.len(),
                        stats: Vec::new(),
                        warnings: Vec::new(),
                        labels,
                    });
                    for source in sources {
                        spawn_build(source, epoch, tx.clone());
                    }
                }
                Job::Built {
                    epoch,
                    source,
                    result,
                    warnings,
                } => {
                    let Some(b) = batch.as_mut().filter(|b| b.epoch == epoch) else {
                        continue; // de una indexación anterior (F5)
                    };
                    b.pending -= 1;
                    b.warnings.extend(warnings);
                    let mut next: Vec<(String, Arc<Index>)> =
                        owners.drain(..).zip(indexes.iter().cloned()).collect();
                    match result {
                        Ok(built) => {
                            b.stats.push(built.stats);
                            let new = Arc::new(built.index);
                            match next.iter_mut().find(|(owner, _)| *owner == source) {
                                Some((_, old)) => *old = new,
                                None => next.push((source, new)),
                            }
                        }
                        // Si no se pudo leer, se queda el índice anterior.
                        Err(e) => b.warnings.push(e),
                    }
                    if b.pending == 0 {
                        next.retain(|(owner, _)| b.labels.contains(owner));
                    }
                    let (o, ix): (Vec<_>, Vec<_>) = next.into_iter().unzip();
                    owners = o;
                    indexes = Arc::new(ix);
                    notify(Event::Indexed {
                        entries: b.stats.iter().map(|s| s.entries).sum(),
                        elapsed: b.started.elapsed(),
                        stats: b.stats.clone(),
                        warnings: b.warnings.clone(),
                        pending: b.pending,
                    });
                    // Lo nuevo entra en las búsquedas que hay en pantalla.
                    for (n, l) in newest.iter_mut().zip(last.iter_mut()) {
                        if n.is_none() {
                            *n = l.take();
                        }
                    }
                }
                Job::Content {
                    client,
                    generation,
                    indexes,
                    mut hits,
                    status,
                    started,
                    key,
                    desc,
                } => {
                    // De una búsqueda ya reemplazada: no se muestra.
                    if scans[client.slot()]
                        .as_ref()
                        .is_none_or(|(g, _)| *g != generation)
                    {
                        continue;
                    }
                    sort_hits(&indexes, &mut hits, key, desc);
                    notify(Event::Results(Results {
                        client,
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
        for (k, s) in newest.into_iter().enumerate() {
            let Some(s) = s else { continue };
            cancel(&mut scans[k]);
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
                    scans[k] = Some((s.generation, Arc::clone(&new)));
                    spawn_content(ContentJob {
                        client: s.client,
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
            last[k] = Some(s);
        }
    }
}

/// Lo que necesita el hilo que revisa el contenido.
struct ContentJob {
    client: Client,
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
                    client: job.client,
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
                source: label,
                result,
                warnings,
            });
        });
    if let Err(e) = spawned {
        let _ = fail_tx.send(Job::Built {
            epoch,
            result: Err(format!("{label}: no se pudo crear un hilo: {e}")),
            source: label,
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
        client: s.client,
        generation: s.generation,
        indexes: Arc::clone(indexes),
        hits,
        elapsed: start.elapsed(),
        error,
        content: None,
    };
    (results, needles)
}
