//! Held-object canonical spelling, including a transient link swap on Unix.
use crate::Directory;
use maestro_test_scratch::scratch_directory;
use std::fs;
#[cfg(unix)]
use std::path::Path;

#[test]
fn canonical_identity_preserves_an_ordinary_held_directory() {
    let root = scratch_directory().unwrap().canonicalize().unwrap();
    let held = Directory::open_canonical(&root).unwrap();
    assert_eq!(held.canonical_path().unwrap(), root);
    drop(held);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn canonical_identity_rejects_transient_link_resolution_to_another_object() {
    use std::os::unix::fs::symlink;
    let root = scratch_directory().unwrap().canonicalize().unwrap();
    let named = root.join("named");
    let moved = root.join("moved");
    let other = root.join("other");
    for directory in [&named, &other] {
        fs::create_dir(directory).unwrap();
    }
    let held = Directory::open_canonical(&named).unwrap();
    let result = held.canonical_path_with(|path| {
        fs::rename(&named, &moved).unwrap();
        symlink(&other, &named).unwrap();
        let resolved = path.canonicalize();
        fs::remove_file(&named).unwrap();
        fs::rename(&moved, &named).unwrap();
        resolved
    });
    assert!(result.is_err());
    assert_eq!(held.canonical_path().unwrap(), named);
    drop(held);
    Directory::open(&root, Path::new("other"), false)
        .unwrap()
        .canonical_path()
        .unwrap();
    fs::remove_dir_all(root).unwrap();
}
