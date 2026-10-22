//! Motor de la ventana: indexa y busca fuera del hilo de la interfaz.

use std::sync::mpsc;
use std::time::Duration;

use lince_core::{SortKey, Source};
use lince_gui::{cell_text, Column, Engine, Event, Results};

const WAIT: Duration = Duration::from_secs(30);

fn engine_with(image: &str) -> (Engine, mpsc::Receiver<Event>) {
    let (tx, rx) = mpsc::channel();
    let engine = Engine::start(move |ev| {
        let _ = tx.send(ev);
    });
    engine.reindex(vec![Source::Image(lince_testdata::image_file(image))]);
    (engine, rx)
}

fn wait_indexed(rx: &mpsc::Receiver<Event>) -> usize {
    loop {
        match rx.recv_timeout(WAIT).expect("sin respuesta del motor") {
            Event::Indexed { entries, .. } => return entries,
            Event::Indexing { .. } => {}
            Event::Results(_) => panic!("resultados antes de indexar"),
        }
    }
}

fn next_results(rx: &mpsc::Receiver<Event>) -> Results {
    loop {
        if let Event::Results(r) = rx.recv_timeout(WAIT).expect("sin resultados") {
            return r;
        }
    }
}

#[test]
fn indexes_then_finds_with_cell_texts() {
    let (engine, rx) = engine_with("basico");
    assert_eq!(wait_indexed(&rx), 21);
    engine.search(1, "canción", SortKey::Name, false, None);
    let r = next_results(&rx);
    assert_eq!(r.generation, 1);
    assert_eq!(r.error, None);
    assert_eq!(r.hits.len(), 1);
    let hit = r.hits[0];
    assert_eq!(cell_text(&r.indexes, hit, Column::Name), "Canción ñ.txt");
    assert_eq!(
        cell_text(&r.indexes, hit, Column::Folder),
        "IMG:\\Docs\\Sub"
    );
    assert_eq!(cell_text(&r.indexes, hit, Column::Size), "8 bytes");
    assert_eq!(
        cell_text(&r.indexes, hit, Column::Modified).len(),
        "22/10/2026 12:57".len()
    );
}

#[test]
fn bad_query_reports_error_and_no_hits() {
    let (engine, rx) = engine_with("basico");
    wait_indexed(&rx);
    engine.search(7, "size:>abc", SortKey::Name, false, None);
    let r = next_results(&rx);
    assert_eq!(r.generation, 7);
    assert!(r.error.unwrap().contains("tamaño inválido"));
    assert!(r.hits.is_empty());
}

#[test]
fn burst_of_searches_ends_with_the_last_one() {
    let (engine, rx) = engine_with("muchos");
    wait_indexed(&rx);
    for g in 1..=40 {
        engine.search(g, &format!("archivo_{g}"), SortKey::Name, false, None);
    }
    let mut last = 0;
    while last < 40 {
        let r = next_results(&rx);
        assert!(r.generation > last, "{} después de {last}", r.generation);
        last = r.generation;
    }
    assert_eq!(last, 40);
}

#[test]
fn sorts_and_filters_by_volume() {
    let (engine, rx) = engine_with("basico");
    wait_indexed(&rx);
    engine.search(1, "file:", SortKey::Size, true, None);
    let r = next_results(&rx);
    assert_eq!(cell_text(&r.indexes, r.hits[0], Column::Name), "grande.bin");
    engine.search(2, "file:", SortKey::Size, true, Some("Z:".into()));
    assert!(next_results(&rx).hits.is_empty());
    engine.search(3, "file:", SortKey::Size, true, Some("IMG:".into()));
    assert!(!next_results(&rx).hits.is_empty());
}

/// Lo que se escribe mientras se indexa se busca solo al terminar el índice.
#[test]
fn search_before_index_runs_when_ready() {
    let (tx, rx) = mpsc::channel();
    let engine = Engine::start(move |ev| {
        let _ = tx.send(ev);
    });
    engine.search(1, "canción", SortKey::Name, false, None);
    engine.reindex(vec![Source::Image(lince_testdata::image_file("basico"))]);
    wait_indexed(&rx);
    let r = next_results(&rx);
    assert_eq!(r.generation, 1);
    assert_eq!(r.hits.len(), 1);
}

/// Al reindexar (F5) se repite la última búsqueda con los datos nuevos.
#[test]
fn reindex_repeats_last_search() {
    let (engine, rx) = engine_with("basico");
    wait_indexed(&rx);
    engine.search(3, "zorro", SortKey::Name, false, None);
    assert_eq!(next_results(&rx).hits.len(), 1);
    engine.reindex(vec![Source::Image(lince_testdata::image_file("basico"))]);
    wait_indexed(&rx);
    let r = next_results(&rx);
    assert_eq!((r.generation, r.hits.len()), (3, 1));
}
