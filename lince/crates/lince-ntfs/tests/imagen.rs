//! Lectura de la MFT de imágenes NTFS reales creadas con mkntfs + ntfs-3g.

use lince_ntfs::{read_layout, read_records, MftScan, ReadOptions};

fn scan(name: &str, threads: usize) -> MftScan {
    let img = lince_testdata::image(name);
    let layout = read_layout(img.as_slice()).unwrap();
    read_records(
        img.as_slice(),
        &layout,
        &ReadOptions {
            threads,
            chunk_bytes: 64 * 1024,
        },
    )
    .unwrap()
}

#[test]
fn layout_of_basic_image() {
    let img = lince_testdata::image("basico");
    let layout = read_layout(img.as_slice()).unwrap();
    assert_eq!(layout.boot.record_size, 1024);
    assert_eq!(layout.boot.bytes_per_cluster, 4096);
    assert!(layout.record_count > 16);
    assert!(!layout.extents.is_empty());
    let total: u64 = layout.extents.iter().map(|e| e.len).sum();
    assert!(total >= layout.record_count * 1024);
}

#[test]
fn fragmented_mft_has_several_extents() {
    let img = lince_testdata::image("muchos");
    let layout = read_layout(img.as_slice()).unwrap();
    assert!(layout.extents.len() > 1, "{:?}", layout.extents);
}

#[test]
fn every_record_parses_cleanly() {
    for name in ["basico", "muchos"] {
        assert_eq!(scan(name, 4).bad_records, 0, "{name}");
    }
}

#[test]
fn finds_unicode_names() {
    let s = scan("basico", 4);
    let names: Vec<&str> = s
        .records
        .iter()
        .flat_map(|r| r.names.iter().map(|n| n.name.as_str()))
        .collect();
    for want in ["Canción ñ.txt", "🦊 zorro.txt", "MAYUS.TXT", "grande.bin"] {
        assert!(names.contains(&want), "falta {want}");
    }
}

#[test]
fn records_are_sorted_and_identical_across_thread_counts() {
    let one = scan("muchos", 1);
    let many = scan("muchos", 8);
    assert!(one.records.windows(2).all(|w| w[0].frs < w[1].frs));
    assert_eq!(one.records, many.records);
}

#[test]
fn unused_records_are_not_read() {
    let img = lince_testdata::image("basico");
    let layout = read_layout(img.as_slice()).unwrap();
    let s = scan("basico", 2);
    assert!(s.bytes_read < layout.record_count * 1024);
}

#[test]
fn hard_link_names_spread_over_extension_records() {
    let s = scan("muchos", 4);
    let base = s
        .records
        .iter()
        .find(|r| r.names.iter().any(|n| n.name == "origen.txt"))
        .expect("origen.txt");
    assert!(base.attr_list.is_some());
    let names = s
        .records
        .iter()
        .filter(|r| r.frs == base.frs || r.base_frs == Some(base.frs))
        .map(|r| r.names.len())
        .sum::<usize>();
    assert_eq!(names, 301);
}
