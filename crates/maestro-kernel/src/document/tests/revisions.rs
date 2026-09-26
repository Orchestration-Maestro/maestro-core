//! Revisions: recorded once with their two artifacts pinned, immutable after
//! but for their status's one move, to failed, and inspectable whatever
//! their status, while a failed one is never eligible.

use super::support::{Scratch, collection, document, failure, ids, pins, revision, source};
use crate::{
    artifact::Digest,
    document::{Error, Recorded, Revision, RevisionStatus},
    store::{self, Database},
};
use rusqlite::ffi;
use serde_json::Value;

/// Writes that would bring the failed revision `rev-a` back as valid, its
/// digests swapped, or free its id, whoever makes them: an insert that
/// replaces it, another, an upsert, and a delete.
const REWRITES: [&str; 4] = [
    "INSERT OR REPLACE INTO revisions (id, document_id, original_digest, canonical_digest,
       status, metadata_json)
     SELECT id, document_id, canonical_digest, original_digest, 'valid', '{}'
     FROM revisions WHERE id = 'rev-a'",
    "REPLACE INTO revisions (id, document_id, original_digest, canonical_digest, status,
       metadata_json)
     SELECT id, document_id, canonical_digest, original_digest, 'valid', '{}'
     FROM revisions WHERE id = 'rev-a'",
    "INSERT INTO revisions (id, document_id, original_digest, canonical_digest, status,
       metadata_json)
     SELECT id, document_id, canonical_digest, original_digest, 'valid', '{}'
     FROM revisions WHERE id = 'rev-a'
     ON CONFLICT (id) DO UPDATE SET status = 'valid'",
    "DELETE FROM revisions WHERE id = 'rev-a'",
];

/// Runs `statement` on the writer and commits what it did.
fn run(database: &Database, statement: &str) -> rusqlite::Result<usize> {
    database
        .write(|transaction| Ok::<_, store::Error>(transaction.execute(statement, [])))
        .unwrap()
}

/// The status column of the revision `id`, as the database holds it.
fn status(database: &Database, id: &str) -> String {
    database
        .reader()
        .unwrap()
        .query_row("SELECT status FROM revisions WHERE id = ?1", [id], |row| {
            row.get(0)
        })
        .unwrap()
}

#[test]
fn a_new_revision_is_recorded_with_its_two_artifacts_pinned() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let given = revision(&database, "rev-a", RevisionStatus::Valid);
    assert_eq!(pins(&database, &given.original_digest), 0);
    assert_eq!(pins(&database, &given.canonical_digest), 0);
    assert_eq!(database.record_revision(&given).unwrap(), Recorded::New);
    assert_eq!(database.revision("rev-a").unwrap(), Some(given.clone()));
    assert_eq!(pins(&database, &given.original_digest), 1);
    assert_eq!(pins(&database, &given.canonical_digest), 1);
    assert_eq!(status(&database, "rev-a"), "valid");
    assert_eq!(database.revision("rev-b").unwrap(), None);
}

#[test]
fn recording_a_revision_again_is_unchanged_and_pins_nothing_more() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let given = revision(&database, "rev-a", RevisionStatus::ValidWithWarnings);
    database.record_revision(&given).unwrap();
    assert_eq!(
        database.record_revision(&given).unwrap(),
        Recorded::Unchanged
    );
    let mut failed_since = given.clone();
    failed_since.status = RevisionStatus::Failed;
    assert_eq!(
        database.record_revision(&failed_since).unwrap(),
        Recorded::Unchanged,
        "the status is no part of a revision's content"
    );
    assert_eq!(pins(&database, &given.original_digest), 1);
    assert_eq!(pins(&database, &given.canonical_digest), 1);
    assert_eq!(database.revision("rev-a").unwrap(), Some(given));
}

#[test]
fn a_revision_recorded_again_with_other_content_is_refused_and_kept() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let kept = revision(&database, "rev-a", RevisionStatus::Valid);
    database.record_revision(&kept).unwrap();
    let other = revision(&database, "rev-b", RevisionStatus::Valid);
    let mut changed = [kept.clone(), kept.clone(), kept.clone(), kept.clone()];
    changed[0].document_id = "doc-b".to_owned();
    changed[1].original_digest = other.original_digest.clone();
    changed[2].canonical_digest = other.canonical_digest.clone();
    changed[3].captured_at = Some("2026-09-13T16:34:20Z".to_owned());
    let mut retitled = kept.clone();
    retitled
        .metadata
        .insert("title".to_owned(), Value::from("Another title"));
    for given in changed.iter().chain([&retitled]) {
        let error = database.record_revision(given).unwrap_err();
        assert!(
            matches!(&error, Error::RevisionConflict(id) if id == "rev-a"),
            "{given:?}: {error:?}"
        );
        assert_eq!(database.revision("rev-a").unwrap().as_ref(), Some(&kept));
    }
    assert_eq!(pins(&database, &kept.original_digest), 1);
    assert_eq!(pins(&database, &other.original_digest), 0);
    assert_eq!(pins(&database, &other.canonical_digest), 0);
}

#[test]
fn a_revision_whose_artifact_is_not_recorded_is_refused_whole() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let mut given = revision(&database, "rev-a", RevisionStatus::Valid);
    let unknown = Digest::of(b"never stored");
    given.canonical_digest = unknown.clone();
    let error = database.record_revision(&given).unwrap_err();
    assert!(
        matches!(&error, Error::Store(store::Error::UnknownArtifact(digest)) if *digest == unknown),
        "{error:?}"
    );
    assert_eq!(database.revision("rev-a").unwrap(), None);
    assert_eq!(pins(&database, &given.original_digest), 0, "rolled back");
}

#[test]
fn a_revision_of_an_unrecorded_document_is_refused() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let mut given = revision(&database, "rev-a", RevisionStatus::Valid);
    given.document_id = "doc-b".to_owned();
    let error = database.record_revision(&given).unwrap_err();
    assert!(
        matches!(&error, Error::Store(store::Error::Sqlite(_))),
        "{error:?}"
    );
    assert_eq!(database.revision("rev-a").unwrap(), None);
    assert_eq!(pins(&database, &given.original_digest), 0);
}

#[test]
fn a_recorded_revision_refuses_every_change_to_its_content() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let kept = revision(&database, "rev-a", RevisionStatus::Valid);
    database.record_revision(&kept).unwrap();
    database
        .record_document(&document("doc-b", "https://example.org/b"))
        .unwrap();
    let changes = [
        "UPDATE revisions SET id = 'rev-b'",
        "UPDATE revisions SET document_id = 'doc-b'",
        "UPDATE revisions SET original_digest = canonical_digest",
        "UPDATE revisions SET canonical_digest = original_digest",
        "UPDATE revisions SET captured_at = '2026-09-13T16:34:20Z'",
        "UPDATE revisions SET metadata_json = '{}'",
        "UPDATE revisions SET recorded_at = '2026-09-13T16:34:20.000Z'",
    ];
    for change in changes {
        let error = run(&database, change).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("a revision is immutable once recorded"),
            "{change}: {error}"
        );
        assert_eq!(database.revision("rev-a").unwrap().as_ref(), Some(&kept));
    }
}

#[test]
fn a_failed_revision_can_be_neither_replaced_nor_deleted() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let failed = revision(&database, "rev-a", RevisionStatus::Failed);
    database.record_revision(&failed).unwrap();
    for rewrite in REWRITES {
        assert_eq!(
            failure(run(&database, rewrite)),
            Some(ffi::SQLITE_CONSTRAINT_TRIGGER),
            "{rewrite}"
        );
        assert_eq!(
            database.revision("rev-a").unwrap().as_ref(),
            Some(&failed),
            "{rewrite}"
        );
        assert_eq!(
            database.eligible_revisions("ctm").unwrap(),
            Vec::new(),
            "{rewrite}"
        );
    }
    assert_eq!(pins(&database, &failed.original_digest), 1);
    assert_eq!(pins(&database, &failed.canonical_digest), 1);
}

#[test]
fn a_revision_status_moves_only_to_failed_and_never_back() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let moves = [
        ("valid", "failed", true),
        ("valid_with_warnings", "failed", true),
        ("valid", "valid_with_warnings", false),
        ("valid_with_warnings", "valid", false),
        ("failed", "valid", false),
        ("failed", "valid_with_warnings", false),
    ];
    for (number, (from, to, allowed)) in moves.into_iter().enumerate() {
        let id = format!("rev-{number}");
        let status_from = match from {
            "valid" => RevisionStatus::Valid,
            "valid_with_warnings" => RevisionStatus::ValidWithWarnings,
            _ => RevisionStatus::Failed,
        };
        database
            .record_revision(&revision(&database, &id, status_from))
            .unwrap();
        let moved = run(
            &database,
            &format!("UPDATE revisions SET status = '{to}' WHERE id = '{id}'"),
        );
        assert_eq!(moved.is_ok(), allowed, "{from} to {to}: {moved:?}");
        let expected = if allowed { to } else { from };
        assert_eq!(status(&database, &id), expected, "{from} to {to}");
    }
}

#[test]
fn a_failed_revision_stays_inspectable_and_is_never_eligible() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let failed = revision(&database, "rev-b", RevisionStatus::Failed);
    database
        .record_revision(&revision(&database, "rev-c", RevisionStatus::Valid))
        .unwrap();
    database.record_revision(&failed).unwrap();
    database
        .record_revision(&revision(
            &database,
            "rev-a",
            RevisionStatus::ValidWithWarnings,
        ))
        .unwrap();
    assert_eq!(database.revision("rev-b").unwrap(), Some(failed.clone()));
    assert_eq!(status(&database, "rev-b"), "failed");
    let eligible = database.eligible_revisions("ctm").unwrap();
    assert_eq!(ids(&eligible), ["rev-c", "rev-a"], "in record order");
    assert_eq!(pins(&database, &failed.original_digest), 1);
    assert_eq!(pins(&database, &failed.canonical_digest), 1);
}

#[test]
fn a_revision_whose_status_moved_to_failed_is_no_longer_eligible() {
    let scratch = Scratch::new();
    let database = scratch.open();
    for id in ["rev-a", "rev-b"] {
        database
            .record_revision(&revision(&database, id, RevisionStatus::Valid))
            .unwrap();
    }
    run(
        &database,
        "UPDATE revisions SET status = 'failed' WHERE id = 'rev-a'",
    )
    .unwrap();
    let eligible = database.eligible_revisions("ctm").unwrap();
    assert_eq!(ids(&eligible), ["rev-b"]);
    let inspected: Option<Revision> = database.revision("rev-a").unwrap();
    assert_eq!(
        inspected.map(|revision| revision.status),
        Some(RevisionStatus::Failed)
    );
}

#[test]
fn eligible_revisions_are_those_of_the_collection_alone() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database
        .record_collection(&collection("synthetic"))
        .unwrap();
    database
        .record_source(&source("synthetic", "docs"))
        .unwrap();
    let mut elsewhere = document("doc-s", "https://example.org/s");
    elsewhere.collection_id = "synthetic".to_owned();
    database.record_document(&elsewhere).unwrap();
    database
        .record_document(&document("doc-b", "https://example.org/b"))
        .unwrap();
    let mut of_synthetic = revision(&database, "rev-s", RevisionStatus::Valid);
    of_synthetic.document_id = "doc-s".to_owned();
    let mut of_doc_b = revision(&database, "rev-b", RevisionStatus::Valid);
    of_doc_b.document_id = "doc-b".to_owned();
    database.record_revision(&of_doc_b).unwrap();
    database.record_revision(&of_synthetic).unwrap();
    database
        .record_revision(&revision(&database, "rev-a", RevisionStatus::Valid))
        .unwrap();
    assert_eq!(
        database.eligible_revisions("ctm").unwrap(),
        [
            of_doc_b,
            revision(&database, "rev-a", RevisionStatus::Valid)
        ]
    );
    assert_eq!(
        ids(&database.eligible_revisions("synthetic").unwrap()),
        ["rev-s"]
    );
    assert_eq!(database.eligible_revisions("other").unwrap(), Vec::new());
}
