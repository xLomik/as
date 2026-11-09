//! Pruebas del ejecutable `lince` con imágenes NTFS reales y carpetas.

use std::process::{Command, Output};

fn lince(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lince-cli"))
        .args(args)
        .output()
        .expect("no se pudo ejecutar lince")
}

fn stdout_lines(out: &Output) -> Vec<String> {
    String::from_utf8(out.stdout.clone())
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

fn image(name: &str) -> String {
    lince_testdata::image_file(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn finds_unicode_name_in_image() {
    let out = lince(&["--image", &image("basico"), "canción"]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(stdout_lines(&out), ["IMG:\\Docs\\Sub\\Canción ñ.txt"]);
}

#[test]
fn count_only() {
    let want = lince_testdata::expected("muchos")
        .iter()
        .filter(|e| !e.is_dir && e.path.to_lowercase().ends_with(".txt"))
        .count();
    let out = lince(&["--image", &image("muchos"), "--count", "*.txt"]);
    assert_eq!(stdout_lines(&out), [want.to_string()]);
}

#[test]
fn sort_desc_and_limit() {
    let out = lince(&[
        "--image",
        &image("basico"),
        "--sort",
        "size",
        "--desc",
        "--limit",
        "1",
        "file:",
    ]);
    assert_eq!(stdout_lines(&out), ["IMG:\\Fotos 2026\\grande.bin"]);
}

#[test]
fn csv_export_for_excel() {
    let dir = tempfile::tempdir().unwrap();
    let csv = dir.path().join("resultados.csv");
    let out = lince(&[
        "--image",
        &image("basico"),
        "--csv",
        csv.to_str().unwrap(),
        "grande.bin",
    ]);
    assert_eq!(out.status.code(), Some(0));
    let text = std::fs::read_to_string(&csv).unwrap();
    let mut lines = text.lines();
    assert_eq!(lines.next(), Some("\u{feff}ruta;tamaño;modificado"));
    let row: Vec<&str> = lines.next().unwrap().split(';').collect();
    assert_eq!(row[0], "IMG:\\Fotos 2026\\grande.bin");
    assert_eq!(row[1], "3000000");
    assert_eq!(row[2].len(), "2026-10-22 12:57:31".len());
    assert_eq!(lines.next(), None);
    // Con --csv no se repiten los resultados en pantalla.
    assert!(stdout_lines(&out).is_empty());
}

#[test]
fn csv_quotes_fields_with_separator() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a;b.txt"), b"x").unwrap();
    let csv = dir.path().join("r.csv");
    lince(&[
        "--path",
        dir.path().to_str().unwrap(),
        "--csv",
        csv.to_str().unwrap(),
        "a;b",
    ]);
    let text = std::fs::read_to_string(&csv).unwrap();
    let row = text.lines().nth(1).unwrap();
    assert!(
        row.starts_with('"') && row.contains("a;b.txt\";1;"),
        "{row}"
    );
}

#[test]
fn walks_a_folder() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("sub")).unwrap();
    std::fs::write(dir.path().join("sub/hola-lince.txt"), b"hola").unwrap();
    let out = lince(&["--path", dir.path().to_str().unwrap(), "hola"]);
    assert_eq!(out.status.code(), Some(0));
    let want = dir.path().join("sub").join("hola-lince.txt");
    assert_eq!(stdout_lines(&out), [want.to_string_lossy().into_owned()]);
}

#[test]
fn no_results_exit_code_is_one() {
    let out = lince(&["--image", &image("basico"), "no-existe-esto"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(out.stdout.is_empty());
}

#[test]
fn invalid_query_is_reported_in_spanish() {
    let out = lince(&["--image", &image("basico"), "size:>abc"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("tamaño inválido"));
}

#[test]
fn stats_go_to_stderr() {
    let out = lince(&["--image", &image("muchos"), "--stats", "origen.txt"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("registros"), "{err}");
    assert!(err.contains("resultados"), "{err}");
    assert_eq!(stdout_lines(&out), ["IMG:\\enlaces\\origen.txt"]);
}

#[test]
fn not_an_ntfs_image_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    let fake = dir.path().join("falsa.img");
    std::fs::write(&fake, vec![0u8; 8192]).unwrap();
    let out = lince(&["--image", fake.to_str().unwrap(), "x"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("NTFS"));
}

#[cfg(not(windows))]
#[test]
fn needs_a_source_outside_windows() {
    let out = lince(&["algo"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&out.stderr).contains("--path"));
}

fn contenido_dir() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/contenido")
        .to_string_lossy()
        .into_owned()
}

fn file_names(out: &Output) -> Vec<String> {
    let mut v: Vec<String> = stdout_lines(out)
        .iter()
        .map(|l| l.rsplit(['/', '\\']).next().unwrap().to_string())
        .collect();
    v.sort();
    v
}

#[test]
fn searches_inside_word_excel_pdf_and_text() {
    let dir = contenido_dir();
    let out = lince(&["--path", &dir, r#"contenido:"orden de compra 4512""#]);
    assert_eq!(out.status.code(), Some(0));
    assert_eq!(
        file_names(&out),
        [
            "documento-97.doc",
            "documento.docx",
            "documento.odt",
            "documento.pdf",
            "navegador.pdf",
            "texto-ansi.txt",
            "texto-utf16.txt",
            "texto-utf8.txt",
        ]
    );
    let out = lince(&["--path", &dir, "ext:xls;xlsx", "content:4512,75"]);
    assert_eq!(file_names(&out), ["hoja-97.xls", "hoja.xlsx"]);
    let out = lince(&["--path", &dir, "contenido:no-está-en-ninguno"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(stdout_lines(&out).is_empty());
}

#[test]
fn content_stats_report_files_read() {
    let out = lince(&["--path", &contenido_dir(), "--stats", "contenido:lince-pdf"]);
    assert_eq!(file_names(&out), ["documento.pdf"]);
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("contenido: 11 archivos revisados"), "{err}");
}
