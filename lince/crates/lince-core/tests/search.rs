//! Consultas sobre un índice pequeño con datos conocidos.

use lince_core::{
    filetime_from_local, parse_query, search, sort_results, Index, IndexBuilder, SortKey, FLAG_DIR,
    FLAG_ROOT, NO_PARENT,
};

const KB: u64 = 1024;
const MB: u64 = 1024 * 1024;

fn t(y: i32, m: u32, d: u32, h: u32, mi: u32) -> u64 {
    filetime_from_local(y, m, d, h, mi, 0).unwrap()
}

fn sample() -> Index {
    let dir_time = t(2026, 6, 1, 9, 0);
    let mut b = IndexBuilder::new("C:", '\\');
    let root = b.push("", NO_PARENT, FLAG_DIR | FLAG_ROOT, 0, dir_time, 0);
    let docs = b.push("Docs", root, FLAG_DIR, 0, dir_time, 0);
    b.push("Canción.TXT", docs, 0, 8, t(2026, 10, 22, 12, 0), 0);
    b.push("informe.pdf", docs, 0, 2 * MB, t(2026, 1, 15, 8, 0), 0);
    b.push("foto.jpg", docs, 0, 500 * KB, t(2025, 12, 31, 23, 30), 0);
    let fotos = b.push("Fotos", root, FLAG_DIR, 0, dir_time, 0);
    b.push(
        "foto vacaciones.JPG",
        fotos,
        0,
        3 * MB,
        t(2026, 7, 4, 10, 0),
        0,
    );
    b.push("notas.txt", fotos, 0, 10, t(2026, 3, 3, 3, 3), 0);
    b.finish()
}

fn names(q: &str) -> Vec<String> {
    let ix = sample();
    let query = parse_query(q).unwrap_or_else(|e| panic!("{q}: {e}"));
    search(&ix, &query)
        .into_iter()
        .map(|i| ix.name(i).to_string())
        .collect()
}

#[test]
fn empty_query_returns_everything_but_root() {
    assert_eq!(names("").len(), 7);
}

#[test]
fn substring_ignores_case_with_unicode() {
    assert_eq!(names("canción"), ["Canción.TXT"]);
    assert_eq!(names("CANCIÓN"), ["Canción.TXT"]);
}

#[test]
fn terms_are_anded() {
    assert_eq!(names("foto jpg"), ["foto.jpg", "foto vacaciones.JPG"]);
}

#[test]
fn quoted_phrase_is_one_term() {
    assert_eq!(names("\"foto vac\""), ["foto vacaciones.JPG"]);
}

#[test]
fn wildcards_match_the_whole_name() {
    assert_eq!(names("*.txt"), ["Canción.TXT", "notas.txt"]);
    assert_eq!(names("foto*"), ["foto.jpg", "Fotos", "foto vacaciones.JPG"]);
    assert_eq!(names("f?to.jpg"), ["foto.jpg"]);
    assert_eq!(names("canci?n.txt"), ["Canción.TXT"]);
}

#[test]
fn negated_term_excludes() {
    assert_eq!(names("foto !vacaciones"), ["foto.jpg", "Fotos"]);
}

#[test]
fn extension_filter() {
    assert_eq!(
        names("ext:jpg;pdf"),
        ["informe.pdf", "foto.jpg", "foto vacaciones.JPG"]
    );
}

#[test]
fn size_filters_apply_to_files() {
    assert_eq!(names("size:>1mb"), ["informe.pdf", "foto vacaciones.JPG"]);
    assert_eq!(names("size:<1kb"), ["Canción.TXT", "notas.txt"]);
    assert_eq!(names("size:400kb..2mb"), ["informe.pdf", "foto.jpg"]);
    assert_eq!(names("size:10"), ["notas.txt"]);
}

#[test]
fn date_filters_use_local_time() {
    assert_eq!(names("dm:2026-10-22"), ["Canción.TXT"]);
    assert_eq!(names("dm:2025"), ["foto.jpg"]);
    assert_eq!(names("dm:2025-12-31"), ["foto.jpg"]);
    assert_eq!(names("dm:<2026"), ["foto.jpg"]);
    assert_eq!(
        names("dm:>=2026-07 file:"),
        ["Canción.TXT", "foto vacaciones.JPG"]
    );
    assert_eq!(names("dm:>2026-07 file:"), ["Canción.TXT"]);
    assert_eq!(
        names("dm:2026-01..2026-03 file:"),
        ["informe.pdf", "notas.txt"]
    );
}

#[test]
fn kind_filters() {
    assert_eq!(names("folder:"), ["Docs", "Fotos"]);
    assert_eq!(names("file: foto"), ["foto.jpg", "foto vacaciones.JPG"]);
}

#[test]
fn path_filter_matches_full_path() {
    assert_eq!(
        names("path:fotos"),
        ["Fotos", "foto vacaciones.JPG", "notas.txt"]
    );
    assert_eq!(names("path:docs\\c"), ["Canción.TXT"]);
}

#[test]
fn regex_filter_ignores_case() {
    assert_eq!(
        names(r"regex:^f.*\.jpg$"),
        ["foto.jpg", "foto vacaciones.JPG"]
    );
}

#[test]
fn invalid_filters_are_reported() {
    for q in ["size:>abc", "size:", "dm:2026-13", "dm:ayer", "regex:("] {
        let err = parse_query(q)
            .err()
            .unwrap_or_else(|| panic!("{q} debería fallar"));
        assert!(!err.to_string().is_empty());
    }
}

#[test]
fn sorting() {
    let ix = sample();
    let mut all = search(&ix, &parse_query("").unwrap());
    sort_results(&ix, &mut all, SortKey::Size, true);
    assert_eq!(ix.name(all[0]), "foto vacaciones.JPG");
    sort_results(&ix, &mut all, SortKey::Name, false);
    assert_eq!(ix.name(all[0]), "Canción.TXT");
    assert_eq!(ix.name(*all.last().unwrap()), "notas.txt");
    sort_results(&ix, &mut all, SortKey::Modified, false);
    assert_eq!(ix.name(all[0]), "foto.jpg");
    sort_results(&ix, &mut all, SortKey::Path, false);
    assert_eq!(ix.path(all[0]), "C:\\Docs");
    assert_eq!(ix.path(all[1]), "C:\\Docs\\Canción.TXT");
}

/// La búsqueda paralela por trozos da lo mismo que filtrar uno por uno.
#[test]
fn parallel_search_matches_naive_filter() {
    let mut b = IndexBuilder::new("C:", '\\');
    let root = b.push("", NO_PARENT, FLAG_DIR | FLAG_ROOT, 0, 0, 0);
    for i in 0..200_000u32 {
        b.push(&format!("Archivo_{i:06}.dat"), root, 0, u64::from(i), 0, 0);
    }
    let ix = b.finish();
    for needle in ["_0123", "9.d", "archivo_19999", "zzz"] {
        let got = search(&ix, &parse_query(needle).unwrap());
        let want: Vec<u32> = (1..ix.len() as u32)
            .filter(|&i| ix.name(i).to_lowercase().contains(needle))
            .collect();
        assert_eq!(got, want, "{needle}");
    }
}

/// Resultados de varios volúmenes ordenados como una sola lista.
#[test]
fn sort_hits_across_sources() {
    use lince_core::{sort_hits, Hit};
    let make = |label: &str, files: &[(&str, u64)]| {
        let mut b = IndexBuilder::new(label, '\\');
        let root = b.push("", NO_PARENT, FLAG_DIR | FLAG_ROOT, 0, 0, 0);
        for &(name, size) in files {
            b.push(name, root, 0, size, 0, 0);
        }
        b.finish()
    };
    let ixs = vec![
        make("D:", &[("b.txt", 5), ("d.txt", 1)]),
        make("C:", &[("a.txt", 3), ("c.txt", 9)]),
    ];
    let paths =
        |hits: &[Hit]| -> Vec<String> { hits.iter().map(|&(s, i)| ixs[s].path(i)).collect() };
    let mut hits: Vec<Hit> = vec![(0, 1), (0, 2), (1, 1), (1, 2)];
    sort_hits(&ixs, &mut hits, SortKey::Name, false);
    assert_eq!(
        paths(&hits),
        ["C:\\a.txt", "D:\\b.txt", "C:\\c.txt", "D:\\d.txt"]
    );
    sort_hits(&ixs, &mut hits, SortKey::Size, true);
    assert_eq!(
        paths(&hits),
        ["C:\\c.txt", "D:\\b.txt", "C:\\a.txt", "D:\\d.txt"]
    );
    sort_hits(&ixs, &mut hits, SortKey::Path, false);
    assert_eq!(
        paths(&hits),
        ["C:\\a.txt", "C:\\c.txt", "D:\\b.txt", "D:\\d.txt"]
    );
}
