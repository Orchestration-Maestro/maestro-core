//! Bounded directory listing on held handles.

use crate::{Directory, EntryKind, tests::scratch};
use std::{fs, io::ErrorKind, path::Path};

#[test]
fn held_listing_accepts_empty_and_exact_bound_refuses_one_past() {
    let root = scratch();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    assert!(directory.list_bounded(0).unwrap().is_empty());
    fs::write(root.join("file"), "data").unwrap();
    fs::create_dir(root.join("folder")).unwrap();
    let entries = directory.list_bounded(2).unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "file");
    assert_eq!(entries[0].kind, EntryKind::File);
    assert_eq!(entries[1].kind, EntryKind::Directory);
    assert!(
        directory.list_bounded(1).is_err(),
        "listing bound must refuse"
    );
    assert_eq!(
        directory.list_bounded(1).unwrap_err().kind(),
        ErrorKind::InvalidData
    );
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

// Unix permits unprivileged links; Windows reparse classification uses held Win32 flags.
#[cfg(unix)]
#[test]
fn held_listing_classifies_links_without_following_them() {
    use std::os::unix::fs::symlink;
    let root = scratch();
    fs::create_dir(root.join("folder")).unwrap();
    symlink(root.join("missing"), root.join("dangling")).unwrap();
    symlink(root.join("folder"), root.join("linked-folder")).unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let entries = directory.list_bounded(3).unwrap();
    assert_eq!(entries[0].name, "dangling");
    assert_eq!(entries[0].kind, EntryKind::Link);
    assert_eq!(entries[2].kind, EntryKind::Link);
    assert_eq!(
        directory.list_bounded(2).unwrap_err().kind(),
        ErrorKind::InvalidData
    );
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn windows_listing_reparse_classification_is_fail_closed() {
    use crate::listing::windows_kind;
    assert_eq!(windows_kind(0x400, true, false), EntryKind::Link);
    assert_eq!(windows_kind(0x400, false, true), EntryKind::Link);
    assert_eq!(windows_kind(0, true, false), EntryKind::Directory);
    assert_eq!(windows_kind(0, false, true), EntryKind::File);
    assert_eq!(windows_kind(0, false, false), EntryKind::Other);
}

#[cfg(unix)]
#[test]
fn held_listing_existing_links_use_no_follow_stat() {
    use std::os::unix::fs::symlink;
    let root = scratch();
    fs::create_dir(root.join("folder")).unwrap();
    symlink(root.join("folder"), root.join("link")).unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let entries = directory.list_bounded(2).unwrap();
    assert_eq!(entries[1].kind, EntryKind::Link);
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}
