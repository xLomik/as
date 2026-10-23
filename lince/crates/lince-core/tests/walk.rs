//! Recorrido de carpetas (respaldo cuando no se puede leer la MFT).

use std::fs;
use std::path::Path;

use lince_core::{walk, Index, FLAG_ROOT};

fn listing(ix: &Index) -> Vec<(String, u64, bool)> {
    let mut v: Vec<_> = (0..ix.len() as u32)
        .filter(|&i| ix.entry(i).flags & FLAG_ROOT == 0)
        .map(|i| {
            let size = if ix.is_dir(i) { 0 } else { ix.entry(i).size };
            (ix.components(i).join("/"), size, ix.is_dir(i))
        })
        .collect();
    v.sort();
    v
}

fn write(path: &Path, bytes: usize) {
    fs::write(path, vec![b'x'; bytes]).unwrap();
}

#[test]
fn indexes_a_directory_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let r = tmp.path();
    fs::create_dir_all(r.join("Docs/Sub")).unwrap();
    fs::create_dir(r.join("Vacía")).unwrap();
    write(&r.join("Docs/a.txt"), 5);
    write(&r.join("Docs/Sub/Canción ñ.txt"), 8);
    write(&r.join("grande.bin"), 300_000);
    for i in 0..500 {
        write(&r.join(format!("Docs/Sub/f{i}.dat")), i);
    }

    let (ix, stats) = walk(r).unwrap();
    let mut want = vec![
        ("Docs".to_string(), 0, true),
        ("Docs/Sub".to_string(), 0, true),
        ("Docs/Sub/Canción ñ.txt".to_string(), 8, false),
        ("Docs/a.txt".to_string(), 5, false),
        ("Vacía".to_string(), 0, true),
        ("grande.bin".to_string(), 300_000, false),
    ];
    want.extend((0..500).map(|i| (format!("Docs/Sub/f{i}.dat"), i as u64, false)));
    want.sort();
    assert_eq!(listing(&ix), want);
    assert_eq!(stats.dirs, 3);
    assert_eq!(stats.files, 503);
    assert_eq!(stats.errors, 0);
}

#[test]
fn paths_start_with_the_walked_folder() {
    let tmp = tempfile::tempdir().unwrap();
    write(&tmp.path().join("x.txt"), 1);
    let (ix, _) = walk(tmp.path()).unwrap();
    let i = (0..ix.len() as u32)
        .find(|&i| ix.name(i) == "x.txt")
        .unwrap();
    assert_eq!(Path::new(&ix.path(i)), tmp.path().join("x.txt"));
}

#[test]
fn missing_root_is_an_error() {
    assert!(walk(Path::new("/no/existe/lince")).is_err());
}

#[test]
fn file_as_root_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    write(&tmp.path().join("x.txt"), 1);
    assert!(walk(&tmp.path().join("x.txt")).is_err());
}

#[cfg(unix)]
#[test]
fn symlinks_are_listed_but_not_followed() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir(tmp.path().join("d")).unwrap();
    // Enlace a la carpeta padre: seguirlo sería un bucle infinito.
    std::os::unix::fs::symlink(tmp.path(), tmp.path().join("d/bucle")).unwrap();
    let (ix, _) = walk(tmp.path()).unwrap();
    let names: Vec<_> = listing(&ix).into_iter().map(|(p, _, d)| (p, d)).collect();
    assert_eq!(
        names,
        vec![("d".to_string(), true), ("d/bucle".to_string(), false)]
    );
}

#[cfg(unix)]
#[test]
fn unreadable_folder_is_counted_and_skipped() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::tempdir().unwrap();
    let locked = tmp.path().join("cerrada");
    fs::create_dir(&locked).unwrap();
    write(&locked.join("secreto.txt"), 1);
    write(&tmp.path().join("visible.txt"), 1);
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
    if fs::read_dir(&locked).is_ok() {
        // root ignora los permisos: la prueba no aplica.
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        eprintln!("ejecutando como root: se omite la prueba de permisos");
        return;
    }
    let (ix, stats) = walk(tmp.path()).unwrap();
    fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(stats.errors, 1);
    let paths: Vec<_> = listing(&ix).into_iter().map(|(p, _, _)| p).collect();
    assert_eq!(paths, vec!["cerrada", "visible.txt"]);
}

/// En red el límite es la latencia: se recorre con más hilos que núcleos, y
/// el resultado debe ser idéntico.
#[test]
fn walking_with_many_threads_gives_the_same_index() {
    use lince_core::walk_with_threads;
    let tmp = tempfile::tempdir().unwrap();
    for d in 0..20 {
        let dir = tmp.path().join(format!("d{d}"));
        fs::create_dir(&dir).unwrap();
        for f in 0..30 {
            write(&dir.join(format!("f{f}.txt")), f);
        }
    }
    let (normal, s1) = walk(tmp.path()).unwrap();
    let (many, s2) = walk_with_threads(tmp.path(), 48).unwrap();
    assert_eq!(listing(&normal), listing(&many));
    assert_eq!(s1, s2);
}
