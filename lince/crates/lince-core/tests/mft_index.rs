//! El índice construido desde la MFT coincide con el sistema de archivos.

use lince_core::{filetime_to_unix, index_from_mft, Index, FLAG_ROOT};
use lince_ntfs::{read_layout, read_records, ReadOptions};
use lince_testdata::Expected;

fn index(name: &str) -> Index {
    let img = lince_testdata::image(name);
    let layout = read_layout(img.as_slice()).unwrap();
    let scan = read_records(
        img.as_slice(),
        &layout,
        &ReadOptions {
            threads: 4,
            chunk_bytes: 256 * 1024,
        },
    )
    .unwrap();
    index_from_mft("X:", scan)
}

fn listing(ix: &Index) -> Vec<Expected> {
    let mut out: Vec<Expected> = (0..ix.len() as u32)
        .filter(|&i| ix.entry(i).flags & FLAG_ROOT == 0)
        .map(|i| Expected {
            path: ix.components(i).join("/"),
            size: if ix.is_dir(i) { 0 } else { ix.entry(i).size },
            is_dir: ix.is_dir(i),
            mtime: filetime_to_unix(ix.entry(i).modified),
        })
        .collect();
    out.sort();
    out
}

fn assert_same(name: &str) {
    let got = listing(&index(name));
    let want = lince_testdata::expected(name);
    let missing: Vec<_> = want.iter().filter(|e| !got.contains(e)).take(5).collect();
    let extra: Vec<_> = got.iter().filter(|e| !want.contains(e)).take(5).collect();
    assert!(
        missing.is_empty() && extra.is_empty(),
        "{name}: faltan {missing:#?}\nsobran {extra:#?}"
    );
    assert_eq!(got.len(), want.len());
}

#[test]
fn basic_image_matches_filesystem() {
    assert_same("basico");
}

#[test]
fn many_files_image_matches_filesystem() {
    assert_same("muchos");
}

#[test]
fn volume_paths_use_backslash() {
    let ix = index("basico");
    let i = (0..ix.len() as u32)
        .find(|&i| ix.name(i) == "profundo.txt")
        .unwrap();
    assert_eq!(ix.path(i), "X:\\a\\b\\c\\d\\e\\f\\g\\h\\profundo.txt");
}
