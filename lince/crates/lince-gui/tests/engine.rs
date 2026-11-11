//! Motor de la ventana: indexa y busca fuera del hilo de la interfaz.

use std::sync::mpsc;
use std::time::Duration;

use lince_core::{SortKey, Source};
use lince_gui::{cell_text, Client, Column, Engine, Event, Results};

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
            Event::Indexed {
                entries,
                pending: 0,
                ..
            } => return entries,
            Event::Indexed { .. } | Event::Indexing { .. } => {}
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

/// Con varias fuentes, cada una aparece en cuanto termina (la red puede
/// tardar mucho más que los discos locales).
#[test]
fn each_source_appears_when_it_finishes() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("en-la-red.txt"), b"x").unwrap();
    let (tx, rx) = mpsc::channel();
    let engine = Engine::start(move |ev| {
        let _ = tx.send(ev);
    });
    engine.reindex(vec![
        Source::Image(lince_testdata::image_file("basico")),
        Source::Path(tmp.path().to_path_buf()),
    ]);
    let mut pendings = Vec::new();
    let total = loop {
        if let Event::Indexed {
            entries, pending, ..
        } = rx.recv_timeout(WAIT).expect("sin respuesta del motor")
        {
            pendings.push(pending);
            if pending == 0 {
                break entries;
            }
        }
    };
    assert_eq!(pendings, vec![1, 0]);
    assert_eq!(total, 21 + 1);
    engine.search(1, "en-la-red", SortKey::Name, false, None);
    let r = next_results(&rx);
    assert_eq!(r.hits.len(), 1);
    assert_eq!(r.indexes.len(), 2);
}

/// En segundo plano Lince vuelve a leer los discos de vez en cuando: mientras
/// tanto se sigue buscando en el índice anterior (nunca queda vacío).
#[test]
fn rereading_keeps_answering_with_the_old_index() {
    let (engine, rx) = engine_with("basico");
    wait_indexed(&rx);
    engine.reindex(vec![Source::Image(lince_testdata::image_file("basico"))]);
    engine.search(1, "zorro", SortKey::Name, false, None);
    let (mut before, mut indexed) = (false, false);
    loop {
        match rx.recv_timeout(WAIT).expect("sin respuesta del motor") {
            Event::Results(r) => {
                assert_eq!(r.hits.len(), 1);
                assert_eq!(r.indexes.len(), 1, "índice repetido");
                if indexed {
                    break;
                }
                before = true;
            }
            Event::Indexed { pending: 0, .. } => indexed = true,
            _ => {}
        }
    }
    // Respondió el índice viejo sin esperar a terminar de leer.
    assert!(before);
}

/// Fuentes que ya no se piden (carpetas quitadas) desaparecen al terminar.
#[test]
fn rereading_drops_sources_no_longer_wanted() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("solo-en-la-carpeta.txt"), b"x").unwrap();
    let (engine, rx) = engine_with("basico");
    wait_indexed(&rx);
    let image = Source::Image(lince_testdata::image_file("basico"));
    engine.reindex(vec![image.clone(), Source::Path(tmp.path().to_path_buf())]);
    wait_indexed(&rx);
    engine.search(1, "solo-en-la-carpeta", SortKey::Name, false, None);
    let r = next_results(&rx);
    assert_eq!((r.hits.len(), r.indexes.len()), (1, 2));
    engine.reindex(vec![image]);
    wait_indexed_after_results(&rx);
    engine.search(2, "solo-en-la-carpeta", SortKey::Name, false, None);
    let r = loop {
        let r = next_results(&rx);
        if r.generation == 2 {
            break r;
        }
    };
    assert_eq!((r.hits.len(), r.indexes.len()), (0, 1));
}

/// El nombre del índice puede no ser igual al de la fuente (`C:` se
/// recorre como `C:\`, una carpeta con barra final pierde la barra): igual
/// debe quedarse.
#[test]
fn a_source_whose_index_label_differs_is_kept() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("con-barra-final.txt"), b"x").unwrap();
    let with_slash = format!("{}{}", tmp.path().display(), std::path::MAIN_SEPARATOR);
    let (tx, rx) = mpsc::channel();
    let engine = Engine::start(move |ev| {
        let _ = tx.send(ev);
    });
    engine.reindex(vec![Source::Path(with_slash.into())]);
    wait_indexed(&rx);
    engine.search(1, "con-barra-final", SortKey::Name, false, None);
    assert_eq!(next_results(&rx).hits.len(), 1);
}

/// Como `wait_indexed`, pero admite resultados de búsquedas repetidas.
fn wait_indexed_after_results(rx: &mpsc::Receiver<Event>) {
    loop {
        if let Event::Indexed { pending: 0, .. } = rx.recv_timeout(WAIT).expect("sin respuesta") {
            return;
        }
    }
}

/// La barra rápida y la ventana buscan a la vez sin pisarse.
#[test]
fn window_and_bar_have_their_own_searches() {
    let (engine, rx) = engine_with("basico");
    wait_indexed(&rx);
    engine.search_from(Client::Bar, 1, "zorro", "", SortKey::Name, false, None);
    engine.search(1, "canción", SortKey::Name, false, None);
    let mut got = Vec::new();
    while got.len() < 2 {
        let r = next_results(&rx);
        let name = cell_text(&r.indexes, r.hits[0], Column::Name);
        got.push((r.client, name));
    }
    got.sort_by_key(|(c, _)| *c == Client::Bar);
    assert_eq!(
        got,
        vec![
            (Client::Window, "Canción ñ.txt".to_string()),
            (Client::Bar, "🦊 zorro.txt".to_string()),
        ]
    );
}

mod content {
    use super::*;
    use std::path::PathBuf;

    fn engine_on_documents() -> (Engine, mpsc::Receiver<Event>) {
        let (tx, rx) = mpsc::channel();
        let engine = Engine::start(move |ev| {
            let _ = tx.send(ev);
        });
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/contenido");
        engine.reindex(vec![Source::Path(dir)]);
        wait_indexed(&rx);
        (engine, rx)
    }

    /// Resultados hasta que termina la revisión del contenido.
    fn until_finished(rx: &mpsc::Receiver<Event>, generation: u64) -> Results {
        loop {
            let r = next_results(rx);
            if r.generation == generation && r.content.as_ref().is_some_and(|c| c.finished) {
                return r;
            }
        }
    }

    fn names(r: &Results) -> Vec<String> {
        let mut v: Vec<String> = r
            .hits
            .iter()
            .map(|&h| cell_text(&r.indexes, h, Column::Name))
            .collect();
        v.sort();
        v
    }

    #[test]
    fn finds_inside_files_with_progress() {
        let (engine, rx) = engine_on_documents();
        engine.search_content(1, "", "LINCE-DOCX", SortKey::Name, false, None);
        let first = next_results(&rx);
        assert_eq!(first.generation, 1);
        assert_eq!(first.content.as_ref().unwrap().total, 11);
        let r = until_finished(&rx, 1);
        assert_eq!(names(&r), ["documento.docx"]);
        let c = r.content.unwrap();
        assert_eq!((c.done, c.total, c.unreadable), (11, 11, 0));
    }

    #[test]
    fn name_filters_and_content_box_combine() {
        let (engine, rx) = engine_on_documents();
        engine.search_content(1, "ext:pdf", "orden de\ncompra", SortKey::Name, false, None);
        let r = until_finished(&rx, 1);
        assert_eq!(names(&r), ["documento.pdf", "navegador.pdf"]);
        assert_eq!(r.content.unwrap().total, 2);
        // Lo mismo escrito en la caja de búsqueda.
        engine.search(
            2,
            r#"ext:pdf contenido:"orden de compra""#,
            SortKey::Name,
            false,
            None,
        );
        assert_eq!(
            names(&until_finished(&rx, 2)),
            ["documento.pdf", "navegador.pdf"]
        );
    }

    #[test]
    fn a_new_search_replaces_the_content_scan() {
        let (engine, rx) = engine_on_documents();
        engine.search_content(1, "", "señalización", SortKey::Name, false, None);
        engine.search(2, "hoja", SortKey::Name, false, None);
        let mut got_2 = false;
        while let Ok(ev) = rx.recv_timeout(Duration::from_secs(2)) {
            if let Event::Results(r) = ev {
                if r.generation == 2 {
                    got_2 = true;
                    assert!(r.content.is_none());
                    assert_eq!(names(&r), ["hoja-97.xls", "hoja.ods", "hoja.xlsx"]);
                } else {
                    assert!(!got_2, "llegó la búsqueda 1 después de la 2");
                }
            }
        }
        assert!(got_2);
    }
}
