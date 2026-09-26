//! Artifacts: stored, recorded with their size, media type and pins, and read
//! back checked.

use super::support::{ABC, Scratch, pins, stored};
use crate::{
    artifact::{self, Digest},
    store::{
        Artifact, Error,
        artifacts::{pin, unpin},
    },
};
use rusqlite::Connection;
use std::{error, fs, io, path::PathBuf};

/// A refusal of the caller's own, as the journal's and the records' will be.
#[derive(Debug)]
enum Refusal {
    /// The database refused.
    Store(Error),
    /// The caller stopped after its pins.
    Stopped,
}

impl From<Error> for Refusal {
    fn from(error: Error) -> Self {
        Self::Store(error)
    }
}

#[test]
fn put_stores_the_bytes_and_records_them_without_a_pin() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let digest = database.put(b"abc", "text/plain").unwrap();
    assert_eq!(digest.as_str(), ABC);
    assert_eq!(
        fs::read(stored(&scratch.artifacts(), &digest)).unwrap(),
        b"abc"
    );
    assert_eq!(database.get(&digest).unwrap(), b"abc");
    let expected = Artifact {
        digest: digest.clone(),
        bytes: 3,
        media: "text/plain".to_owned(),
        pins: 0,
    };
    assert_eq!(database.artifact(&digest).unwrap(), Some(expected));
    assert_eq!(database.artifact(&Digest::of(b"")).unwrap(), None);
}

#[test]
fn bytes_that_cannot_be_stored_are_never_recorded() {
    let scratch = Scratch::new();
    let database = scratch.open();
    // A file where the store's first directory goes.
    fs::create_dir(scratch.artifacts()).unwrap();
    fs::write(scratch.artifacts().join("sha256"), b"").unwrap();
    let error = database.put(b"abc", "text/plain").unwrap_err();
    assert!(
        matches!(error, Error::Artifact(artifact::Error::Io { .. })),
        "{error:?}"
    );
    assert_eq!(database.artifact(&Digest::of(b"abc")).unwrap(), None);
}

#[test]
fn put_records_the_time_the_artifact_was_first_stored() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let clock = Connection::open_in_memory().unwrap();
    let now = || -> String {
        clock
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ', 'now')", [], |row| {
                row.get(0)
            })
            .unwrap()
    };
    let before = now();
    database.put(b"abc", "text/plain").unwrap();
    let after = now();
    let created: String = database
        .reader()
        .unwrap()
        .query_row(
            "SELECT created_at FROM artifacts WHERE digest = ?1",
            [ABC],
            |row| row.get(0),
        )
        .unwrap();
    assert!(
        before <= created && created <= after,
        "{before} <= {created} <= {after}"
    );
}

#[test]
fn putting_pinned_bytes_again_keeps_their_pins() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let digest = database.put(b"abc", "text/plain").unwrap();
    database.pin(&digest).unwrap();
    assert_eq!(database.put(b"abc", "text/plain").unwrap(), digest);
    assert_eq!(pins(&database, &digest), Some(1));
}

#[test]
fn the_same_bytes_under_another_media_type_are_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let digest = database.put(b"abc", "text/plain").unwrap();
    let error = database.put(b"abc", "text/markdown").unwrap_err();
    assert!(
        matches!(
            &error,
            Error::MediaConflict { digest: found, recorded, given }
                if *found == digest && recorded == "text/plain" && given == "text/markdown"
        ),
        "{error:?}"
    );
    let recorded = database.artifact(&digest).unwrap().unwrap();
    assert_eq!(recorded.media, "text/plain", "the record is unchanged");
}

#[test]
fn pins_count_up_and_down_to_zero_but_not_below() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let digest = database.put(b"abc", "text/plain").unwrap();
    database.pin(&digest).unwrap();
    database.pin(&digest).unwrap();
    assert_eq!(pins(&database, &digest), Some(2));
    database.unpin(&digest).unwrap();
    assert_eq!(pins(&database, &digest), Some(1));
    database.unpin(&digest).unwrap();
    assert_eq!(pins(&database, &digest), Some(0));
    let error = database.unpin(&digest).unwrap_err();
    assert!(
        matches!(&error, Error::NotPinned(found) if *found == digest),
        "{error:?}"
    );
    assert_eq!(pins(&database, &digest), Some(0));
}

#[test]
fn pins_taken_inside_a_write_that_then_fails_are_rolled_back() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let digest = database.put(b"abc", "text/plain").unwrap();
    let refused = database.write(|transaction| {
        pin(transaction, &digest)?;
        pin(transaction, &digest)?;
        unpin(transaction, &digest)?;
        let inside: i64 = transaction
            .query_row(
                "SELECT pins FROM artifacts WHERE digest = ?1",
                [ABC],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(inside, 1, "the pins are in the transaction");
        Err::<(), _>(Refusal::Stopped)
    });
    assert!(matches!(refused, Err(Refusal::Stopped)), "{refused:?}");
    assert_eq!(pins(&database, &digest), Some(0), "they went with it");
    let unknown = Digest::of(b"never stored");
    let refused = database.write(|transaction| {
        pin(transaction, &unknown)?;
        Ok::<_, Refusal>(())
    });
    assert!(
        matches!(&refused, Err(Refusal::Store(Error::UnknownArtifact(found))) if *found == unknown),
        "{refused:?}"
    );
}

#[test]
fn pinning_an_artifact_never_recorded_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let unknown = Digest::of(b"never stored");
    let pinned = database.pin(&unknown).unwrap_err();
    assert!(
        matches!(&pinned, Error::UnknownArtifact(found) if *found == unknown),
        "{pinned:?}"
    );
    let unpinned = database.unpin(&unknown).unwrap_err();
    assert!(
        matches!(&unpinned, Error::UnknownArtifact(found) if *found == unknown),
        "{unpinned:?}"
    );
    assert_eq!(database.artifact(&unknown).unwrap(), None);
}

#[test]
fn get_reports_what_the_artifact_store_reports() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let unknown = Digest::of(b"never stored");
    let error = database.get(&unknown).unwrap_err();
    assert!(
        matches!(&error, Error::Artifact(artifact::Error::Missing(found)) if *found == unknown),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        artifact::Error::Missing(unknown).to_string()
    );
}

#[test]
fn every_error_says_what_went_wrong() {
    let digest = Digest::of(b"abc");
    let unknown = Error::UnknownMigration("0099_future".to_owned()).to_string();
    assert!(unknown.contains("0099_future"), "{unknown}");
    let messages = [
        Error::UnknownArtifact(digest.clone()).to_string(),
        Error::NotPinned(digest.clone()).to_string(),
    ];
    for message in &messages {
        assert!(message.contains(ABC), "{message}");
    }
    assert_ne!(messages[0], messages[1]);
    let conflict = Error::MediaConflict {
        digest: digest.clone(),
        recorded: "text/plain".to_owned(),
        given: "text/markdown".to_owned(),
    }
    .to_string();
    for part in [ABC, "text/plain", "text/markdown"] {
        assert!(
            conflict.contains(part),
            "{part} is missing from: {conflict}"
        );
    }
    for error in [
        Error::UnknownMigration(String::new()),
        Error::UnknownArtifact(digest.clone()),
        Error::NotPinned(digest.clone()),
    ] {
        assert!(error::Error::source(&error).is_none(), "{error}");
    }
    // A wrapped artifact error is its own message and its own source.
    let path = PathBuf::from("sha256");
    let inner = || artifact::Error::Io {
        path: path.clone(),
        source: io::Error::other("the disk is full"),
    };
    let wrapped = Error::from(inner());
    assert_eq!(wrapped.to_string(), inner().to_string());
    let reason = error::Error::source(&wrapped).map(ToString::to_string);
    assert_eq!(reason.as_deref(), Some("the disk is full"));
}
