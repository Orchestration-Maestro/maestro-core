//! Discovery distinguishes unusable starts, malformed approval and unsafe markers.
use crate::{
    limits::Limits,
    settings::{NoWorkspaceTrust, SessionPreferences, WorkspaceTrust},
};
use maestro_test_scratch::scratch_directory;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Deliberately malformed approval, to test the consumer's containing-root check.
struct Elsewhere(PathBuf);
impl WorkspaceTrust for Elsewhere {
    fn containing_root(&self, _: &Path) -> Option<PathBuf> {
        Some(self.0.clone())
    }
}

#[test]
fn discovery_refuses_file_starts_and_noncontaining_approval_with_named_notes() {
    let root = scratch_directory().unwrap();
    let file = root.join("file");
    fs::write(&file, b"not a workspace").unwrap();
    let snapshot = SessionPreferences::load_for_init(
        &root.join("user"),
        Some(&file),
        Some(&root),
        &NoWorkspaceTrust,
        &Limits::PRODUCTION,
    )
    .unwrap();
    assert_eq!(
        snapshot.discovery.note.as_deref(),
        Some("workspace cannot be resolved as a directory")
    );
    let sibling = root.join("sibling");
    let start = root.join("start");
    fs::create_dir(&sibling).unwrap();
    fs::create_dir(&start).unwrap();
    let snapshot = SessionPreferences::load_for_init(
        &root.join("user"),
        Some(&start),
        None,
        &Elsewhere(sibling.canonicalize().unwrap()),
        &Limits::PRODUCTION,
    )
    .unwrap();
    assert!(
        snapshot
            .discovery
            .note
            .unwrap()
            .contains("maestro trust add")
    );
    assert_eq!(NoWorkspaceTrust.containing_root(&root), None);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_non_directory_lock_container_is_not_silently_absent() {
    let root = scratch_directory().unwrap();
    let start = root.join("project");
    fs::create_dir(&start).unwrap();
    fs::write(start.join(".maestro"), b"not a directory").unwrap();
    let snapshot = SessionPreferences::load(
        &root.join("user"),
        Some(&start),
        Some(&root),
        &NoWorkspaceTrust,
        &Limits::PRODUCTION,
    )
    .unwrap();
    assert!(
        snapshot
            .lock
            .unwrap()
            .unwrap_err()
            .contains("authoring.lock.json")
    );
    fs::remove_dir_all(root).unwrap();
}
