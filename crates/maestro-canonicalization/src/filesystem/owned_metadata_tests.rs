//! Read-only receipt and staging metadata checks preserve unrelated bytes.
use super::OwnedRoot;
use maestro_test_scratch::scratch_directory;
use std::{fs, io};

#[test]
fn filesystem_receipt_metadata_refuses_missing_unsafe_names_and_aliases_read_only() {
    let scratch = scratch_directory().unwrap();
    let root = OwnedRoot::open(&scratch, false).unwrap();
    root.make_private().unwrap();
    fs::write(scratch.join("valid.lbdb"), b"valid neighbour").unwrap();
    root.check_regular("valid.lbdb").unwrap();
    assert_eq!(
        root.check_regular("missing.lbdb").unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    for name in [
        "",
        ".",
        "..",
        "../outside",
        "/outside",
        "nested/graph",
        "graph:stream",
        "graph\\leaf",
    ] {
        assert_eq!(
            root.check_regular(name).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
    fs::create_dir(scratch.join("directory")).unwrap();
    assert!(root.check_regular("directory").is_err());
    fs::hard_link(scratch.join("valid.lbdb"), scratch.join("alias.lbdb")).unwrap();
    assert!(root.check_regular("alias.lbdb").is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(scratch.join("valid.lbdb"), scratch.join("linked.lbdb")).unwrap();
        assert!(root.check_regular("linked.lbdb").is_err());
    }
    assert_eq!(
        fs::read(scratch.join("valid.lbdb")).unwrap(),
        b"valid neighbour"
    );
    drop(root);
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn filesystem_publication_rejects_nonchild_staging() {
    let scratch = scratch_directory().unwrap();
    let root = OwnedRoot::open(&scratch.join("owned"), true).unwrap();
    let staging = OwnedRoot::open(&scratch.join("staging"), true).unwrap();
    let name = format!("g{}.lbdb", "a".repeat(64));
    let source = staging.resolved_path().unwrap().join(&name);
    let destination = root.resolved_path().unwrap().join(&name);
    fs::write(&source, b"private valid source").unwrap();
    assert_eq!(
        root.install_from(&staging, &name).unwrap_err().to_string(),
        "publication staging must be a reserved direct child"
    );
    assert_eq!(fs::read(&source).unwrap(), b"private valid source");
    assert!(!destination.exists());
    assert_eq!(
        fs::read_dir(root.resolved_path().unwrap()).unwrap().count(),
        0
    );
    drop((root, staging));
    fs::remove_dir_all(scratch).unwrap();
}
