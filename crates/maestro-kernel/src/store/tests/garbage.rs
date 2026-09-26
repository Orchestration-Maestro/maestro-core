//! Garbage collection: it lists before it removes, removes only artifacts
//! with no pin, leaves temporary files alone, and keeps every artifact a put
//! brings back while it runs, whatever the order of the two.

use super::support::{ABC, EMPTY, HELLO, Scratch, pins, stored};
use crate::{
    artifact::{self, Digest},
    store::{Artifact, Database, Error},
};
use std::{fs, path::PathBuf};

/// The digests of `artifacts`, in their order.
fn digests(artifacts: &[Artifact]) -> Vec<&str> {
    artifacts
        .iter()
        .map(|artifact| artifact.digest.as_str())
        .collect()
}

/// Puts a directory in place of the file of `digest`, which no removal of a
/// file can then remove, and returns its path.
fn block(scratch: &Scratch, digest: &Digest) -> PathBuf {
    let artifact = stored(&scratch.artifacts(), digest);
    fs::remove_file(&artifact).unwrap();
    fs::create_dir_all(artifact.join("occupied")).unwrap();
    artifact
}

/// A database holding `abc`, pinned, then the empty input and `hello`,
/// neither pinned: an order apart from the digests' own.
fn three_artifacts(scratch: &Scratch) -> Database {
    let database = scratch.open();
    let pinned = database.put(b"abc", "text/plain").unwrap();
    database.pin(&pinned).unwrap();
    database.put(b"", "text/plain").unwrap();
    database.put(b"hello", "text/plain").unwrap();
    database
}

#[test]
fn a_dry_run_lists_the_unpinned_artifacts_and_removes_nothing() {
    let scratch = Scratch::new();
    let database = three_artifacts(&scratch);
    let garbage = database.garbage().unwrap();
    assert_eq!(digests(&garbage), [HELLO, EMPTY], "in digest order");
    assert_eq!(garbage[0].bytes, 5);
    assert_eq!(garbage[0].media, "text/plain");
    for hex in [ABC, EMPTY, HELLO] {
        let digest = Digest::parse(hex).unwrap();
        assert!(database.artifact(&digest).unwrap().is_some(), "{hex}");
        assert!(database.get(&digest).is_ok(), "{hex}");
    }
}

#[test]
fn a_collection_removes_the_rows_and_files_of_unpinned_artifacts_only() {
    let scratch = Scratch::new();
    let database = three_artifacts(&scratch);
    let removed = database.collect_garbage().unwrap();
    assert_eq!(digests(&removed), [HELLO, EMPTY]);
    for hex in [HELLO, EMPTY] {
        let digest = Digest::parse(hex).unwrap();
        assert_eq!(database.artifact(&digest).unwrap(), None, "{hex}");
        assert!(!stored(&scratch.artifacts(), &digest).exists(), "{hex}");
        assert!(
            matches!(
                database.get(&digest),
                Err(Error::Artifact(artifact::Error::Missing(_)))
            ),
            "{hex}"
        );
    }
    let kept = Digest::parse(ABC).unwrap();
    assert_eq!(database.get(&kept).unwrap(), b"abc");
    assert_eq!(pins(&database, &kept), Some(1));
    assert!(database.collect_garbage().unwrap().is_empty());
}

#[test]
fn a_collection_leaves_temporary_files_alone() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let digest = database.put(b"hello", "text/plain").unwrap();
    let artifact = stored(&scratch.artifacts(), &digest);
    let temporary = artifact.with_file_name(".tmp-7-0");
    fs::write(&temporary, b"a write in progress").unwrap();
    assert_eq!(digests(&database.collect_garbage().unwrap()), [HELLO]);
    assert!(!artifact.exists());
    assert_eq!(fs::read(&temporary).unwrap(), b"a write in progress");
}

#[test]
fn a_pin_taken_after_the_listing_keeps_the_artifact() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let digest = database.put(b"hello", "text/plain").unwrap();
    let garbage = database.garbage().unwrap();
    database.pin(&digest).unwrap();
    assert!(database.delete_rows(&garbage).unwrap().is_empty());
    assert_eq!(pins(&database, &digest), Some(1));
    assert_eq!(database.get(&digest).unwrap(), b"hello");
}

#[test]
fn a_put_between_the_row_deletion_and_the_unlink_keeps_the_file() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let digest = database.put(b"hello", "text/plain").unwrap();
    let deleted = database.delete_rows(&database.garbage().unwrap()).unwrap();
    assert_eq!(digests(&deleted), [HELLO]);
    assert_eq!(pins(&database, &digest), None);
    database.put(b"hello", "text/plain").unwrap();
    database.remove_unrecorded(&digest).unwrap();
    assert_eq!(database.get(&digest).unwrap(), b"hello");
    assert_eq!(pins(&database, &digest), Some(0));
}

#[test]
fn a_put_after_the_unlink_writes_the_file_again() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let digest = database.put(b"hello", "text/plain").unwrap();
    database.delete_rows(&database.garbage().unwrap()).unwrap();
    database.remove_unrecorded(&digest).unwrap();
    assert!(!stored(&scratch.artifacts(), &digest).exists());
    database.put(b"hello", "text/plain").unwrap();
    assert_eq!(database.get(&digest).unwrap(), b"hello");
    assert_eq!(pins(&database, &digest), Some(0));
}

#[test]
fn a_file_collected_before_a_put_records_its_row_is_stored_again() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let digest = database.put(b"hello", "text/plain").unwrap();
    database.delete_rows(&database.garbage().unwrap()).unwrap();
    // A put's first store finds the bytes in place; the collection then
    // removes them, since no row names them yet.
    database.artifacts.put(b"hello").unwrap();
    database.remove_unrecorded(&digest).unwrap();
    assert!(!stored(&scratch.artifacts(), &digest).exists());
    database
        .record_then_store(b"hello", &digest, "text/plain")
        .unwrap();
    assert_eq!(database.get(&digest).unwrap(), b"hello");
    assert_eq!(pins(&database, &digest), Some(0));
}

#[test]
fn an_artifact_whose_file_is_already_gone_is_still_collected() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let digest = database.put(b"hello", "text/plain").unwrap();
    fs::remove_file(stored(&scratch.artifacts(), &digest)).unwrap();
    assert_eq!(digests(&database.collect_garbage().unwrap()), [HELLO]);
    assert_eq!(pins(&database, &digest), None);
}

#[test]
fn files_that_cannot_be_removed_fail_the_collection_once_it_tried_every_file() {
    let scratch = Scratch::new();
    let database = scratch.open();
    // In digest order: `hello`, then `abc`, then the empty input.
    let first = database.put(b"hello", "text/plain").unwrap();
    let removable = database.put(b"abc", "text/plain").unwrap();
    let last = database.put(b"", "text/plain").unwrap();
    let blocked = [block(&scratch, &first), block(&scratch, &last)];
    let error = database.collect_garbage().unwrap_err();
    assert!(
        matches!(&error, Error::Artifact(artifact::Error::Io { path, .. }) if *path == blocked[0]),
        "the first failure is reported: {error:?}"
    );
    for digest in [&first, &removable, &last] {
        assert_eq!(pins(&database, digest), None, "{}", digest.as_str());
    }
    assert!(
        !stored(&scratch.artifacts(), &removable).exists(),
        "a file after the failure is removed"
    );
    assert!(
        blocked.iter().all(|path| path.is_dir()),
        "what cannot be removed stays, a file no row names"
    );
}
