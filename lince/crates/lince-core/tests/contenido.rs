//! Búsqueda en el contenido con documentos reales de `fixtures/contenido`
//! (Word, Excel, OpenDocument y PDF hechos con LibreOffice y Chromium, y
//! textos en UTF-8, UTF-16 y ANSI). Ver `tools/gen-fixtures-contenido.sh`.

use std::path::{Path, PathBuf};

use lince_core::content::{extract_text, file_contains, format_of, needle, normalize, Format};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/contenido")
        .join(name)
}

fn text_of(name: &str) -> String {
    let path = fixture(name);
    let raw = extract_text(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
    String::from_utf8(normalize(&raw)).unwrap()
}

const DOCUMENTS: [(&str, &str); 9] = [
    ("documento.docx", "lince-docx"),
    ("documento-97.doc", "lince-doc"),
    ("documento.odt", "lince-odt"),
    ("documento.pdf", "lince-pdf"),
    ("navegador.pdf", "lince-navegador"),
    ("texto-utf8.txt", "lince-utf8"),
    ("texto-utf16.txt", "lince-utf16"),
    ("texto-ansi.txt", "lince-ansi"),
    ("hoja.xlsx", "lince-xlsx"),
];

#[test]
fn every_format_yields_its_text_with_accents() {
    for (name, marker) in DOCUMENTS {
        let text = text_of(name);
        assert!(text.contains(marker), "{name}: falta {marker} en {text:?}");
        if !name.starts_with("hoja") {
            assert!(text.contains("señalización del camión"), "{name}: {text:?}");
            // En los textos la frase está partida en dos líneas.
            assert!(text.contains("orden de compra 4512"), "{name}: {text:?}");
        }
    }
}

#[test]
fn spreadsheets_old_and_new() {
    for (name, marker) in [
        ("hoja.xlsx", "lince-xlsx"),
        ("hoja-97.xls", "lince-xls"),
        ("hoja.ods", "lince-ods"),
    ] {
        let text = text_of(name);
        assert!(text.contains(marker), "{name}: {text:?}");
        assert!(
            text.contains("lámina de acero inoxidable"),
            "{name}: {text:?}"
        );
        // Los números se encuentran con punto o con coma decimal.
        assert!(text.contains("4512.75"), "{name}: {text:?}");
        assert!(text.contains("4512,75"), "{name}: {text:?}");
    }
}

/// Word guarda «par» en cursiva y «tido» normal como trozos separados: la
/// palabra debe quedar entera.
#[test]
fn words_split_by_formatting_stay_whole() {
    for name in [
        "documento.docx",
        "documento-97.doc",
        "documento.odt",
        "documento.pdf",
    ] {
        let text = text_of(name);
        assert!(
            text.contains("texto partido en formatos"),
            "{name}: {text:?}"
        );
        assert!(text.contains("contrato marco de 2026"), "{name}: {text:?}");
    }
}

#[test]
fn search_ignores_case_and_line_breaks() {
    let n = |s: &str| needle(s);
    for (name, marker) in DOCUMENTS {
        let path = fixture(name);
        let upper = marker.to_uppercase();
        assert!(file_contains(&path, &[n(&upper)]).unwrap(), "{name}");
        assert!(!file_contains(&path, &[n(&upper), n("no-está-aquí")]).unwrap());
        if !name.starts_with("hoja") {
            // En los textos, «orden de» y «compra» van en líneas distintas.
            let both = [n("ORDEN DE COMPRA"), n("Señalización")];
            assert!(file_contains(&path, &both).unwrap(), "{name}");
        }
    }
}

#[test]
fn formats_by_extension() {
    let f = |p: &str| format_of(Path::new(p));
    assert_eq!(f(r"C:\a\Informe.DOCX"), Some(Format::OfficeXml));
    assert_eq!(f("x.pptx"), Some(Format::OfficeXml));
    assert_eq!(f("x.doc"), Some(Format::WordBinary));
    assert_eq!(f("x.xls"), Some(Format::Spreadsheet));
    assert_eq!(f("x.xlsx"), Some(Format::Spreadsheet));
    assert_eq!(f("x.ods"), Some(Format::Spreadsheet));
    assert_eq!(f("x.odt"), Some(Format::OpenDocument));
    assert_eq!(f("x.pdf"), Some(Format::Pdf));
    assert_eq!(f("x.txt"), Some(Format::Text));
    assert_eq!(f("pieza.nc"), Some(Format::Text));
    assert_eq!(f("plano.dxf"), Some(Format::Text));
    assert_eq!(f("binario.dat"), None);
    assert_eq!(f("foto.jpg"), None);
    assert_eq!(f("sin-extension"), None);
}

#[test]
fn broken_files_are_errors_not_crashes() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["roto.docx", "roto.xlsx", "roto.pdf", "roto.doc", "roto.odt"] {
        let path = dir.path().join(name);
        std::fs::write(&path, b"esto no es un documento de verdad").unwrap();
        assert!(extract_text(&path).is_err(), "{name}");
        assert!(file_contains(&path, &[needle("verdad")]).is_err(), "{name}");
    }
    // Un documento a medias (cortado al copiar) tampoco rompe nada.
    let cut = dir.path().join("cortado.pdf");
    let full = std::fs::read(fixture("documento.pdf")).unwrap();
    std::fs::write(&cut, &full[..full.len() / 2]).unwrap();
    let _ = file_contains(&cut, &[needle("orden")]);
}

mod scan {
    use std::path::PathBuf;

    use lince_core::content::{content_candidates, ContentScan};
    use lince_core::{parse_query, search, walk, Hit, Index};

    fn fixtures_index() -> Index {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/contenido");
        walk(&dir).unwrap().0
    }

    fn names(ix: &Index, hits: &[Hit]) -> Vec<String> {
        let mut v: Vec<String> = hits.iter().map(|&(_, i)| ix.name(i).to_string()).collect();
        v.sort();
        v
    }

    fn find(text: &str) -> Vec<String> {
        let ix = fixtures_index();
        let q = parse_query(text).unwrap();
        let hits: Vec<Hit> = search(&ix, &q).into_iter().map(|i| (0, i)).collect();
        let candidates = content_candidates(&[&ix], &hits);
        let scan = ContentScan::new();
        scan.run(&[&ix], &candidates, &q.content, 4);
        assert_eq!(scan.done(), candidates.len());
        names(&ix, &scan.found())
    }

    #[test]
    fn query_with_content_finds_only_matching_files() {
        assert_eq!(find("contenido:lince-docx"), ["documento.docx"]);
        assert_eq!(
            find(r#"content:"LÁMINA DE ACERO""#),
            ["hoja-97.xls", "hoja.ods", "hoja.xlsx"]
        );
        // Nombre y contenido a la vez.
        assert_eq!(
            find(r#"ext:pdf contenido:"orden de compra""#),
            ["documento.pdf", "navegador.pdf"]
        );
        assert_eq!(find("texto contenido:señalización").len(), 3);
        assert!(find("contenido:no-está-en-ningún-archivo").is_empty());
    }

    #[test]
    fn only_readable_files_are_candidates() {
        let ix = fixtures_index();
        let all: Vec<Hit> = (0..ix.len() as u32).map(|i| (0, i)).collect();
        let c = names(&ix, &content_candidates(&[&ix], &all));
        assert!(
            !c.iter().any(|n| n == "binario.dat" || n.is_empty()),
            "{c:?}"
        );
        assert_eq!(c.len(), 11, "{c:?}");
    }

    #[test]
    fn a_cancelled_scan_stops() {
        let ix = fixtures_index();
        let all: Vec<Hit> = (0..ix.len() as u32).map(|i| (0, i)).collect();
        let candidates = content_candidates(&[&ix], &all);
        let scan = ContentScan::new();
        scan.cancel();
        scan.run(
            &[&ix],
            &candidates,
            &[lince_core::content::needle("orden")],
            2,
        );
        assert!(scan.found().is_empty());
        assert!(scan.is_cancelled());
    }
}
