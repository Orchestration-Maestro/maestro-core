//! What the gate reports, as JSON: every revision counted once by outcome
//! and by rule, the ledger rules kept dispositions outranked, and each one
//! held back listed; and why it stops, before any work for a collection its
//! caller cannot read, part way for a canonical document the kernel cannot
//! give back.

use super::support::{CLEAN, EMPTY, GATE, REPLACED, Scratch, disposition_of, revision_of};
use maestro_kernel::{
    artifact::Digest,
    document::{self, Document, Revision, RevisionStatus},
    scope::Right,
    store,
};
use maestro_knowledge::quality::{self, Error, Ledger};
use serde_json::{Map, json};
use std::{error::Error as _, fs};

#[test]
fn the_report_counts_every_revision_once_and_lists_each_one_held_back() {
    let scratch = Scratch::new();
    scratch.pages(&[("clean", CLEAN), ("empty", EMPTY), ("replaced", REPLACED)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    let report = quality::gate(&database, &scopes, "garden", &Ledger::default()).unwrap();
    let empty = revision_of(&database, &scopes, EMPTY);
    let reasons = disposition_of(&database, &scopes, EMPTY).reasons;
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        json!({
            "collection": "garden",
            "revisions": 3,
            "decided": 3,
            "kept": 0,
            "outcomes": {
                "accepted": 1,
                "accepted_with_warnings": 1,
                "needs_reextraction": 1,
                "quarantined": 0,
                "excluded": 0,
            },
            "rules": { "body.near-empty": 1, "text.replacement-characters": 1 },
            "ignored_rules": {},
            "held": [{
                "revision": empty.id,
                "document": empty.document_id,
                "source_ref": "https://example.org/empty",
                "source_kind": "guide",
                "set": "notes",
                "outcome": "needs_reextraction",
                "rule_ids": ["body.near-empty"],
                "reasons": reasons,
                "decided_by": GATE,
            }],
        })
    );
}

#[test]
fn a_collection_its_caller_cannot_read_is_refused_before_any_work() {
    let scratch = Scratch::new();
    scratch.pages(&[("clean", CLEAN)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    let orchard = "workspace/default/collection/orchard".parse().unwrap();
    database
        .grant("gardener", &orchard, Right::Read, "test")
        .unwrap();
    let elsewhere = database.visible("gardener").unwrap();
    for (scopes, collection) in [(&elsewhere, "garden"), (&scopes, "orchard")] {
        let refused = quality::gate(&database, scopes, collection, &Ledger::default());
        let Err(Error::UnknownCollection(named)) = &refused else {
            panic!("{refused:?}");
        };
        assert_eq!(named, collection);
    }
    let revision = revision_of(&database, &scopes, CLEAN);
    assert_eq!(database.disposition(&scopes, &revision.id).unwrap(), None);
}

#[test]
fn a_canonical_document_the_kernel_cannot_give_back_stops_the_gate() {
    let scratch = Scratch::new();
    scratch.pages(&[("clean", CLEAN), ("empty", EMPTY)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    let broken = Revision {
        id: "rev-broken".to_owned(),
        document_id: "doc-broken".to_owned(),
        original_digest: database.put(b"# Broken\n", "text/markdown").unwrap(),
        canonical_digest: database.put(b"{}", "application/json").unwrap(),
        status: RevisionStatus::Valid,
        captured_at: None,
        metadata: Map::new(),
    };
    database
        .record_document(&Document {
            id: broken.document_id.clone(),
            collection_id: "garden".to_owned(),
            source_id: "docs".to_owned(),
            source_ref: "https://example.org/broken".to_owned(),
        })
        .unwrap();
    database.record_revision(&broken).unwrap();
    let refused = quality::gate(&database, &scopes, "garden", &Ledger::default());
    let Err(Error::Canonical { revision_id, .. }) = &refused else {
        panic!("{refused:?}");
    };
    assert_eq!(revision_id, "rev-broken");
    assert!(
        database
            .disposition(&scopes, &revision_of(&database, &scopes, CLEAN).id)
            .unwrap()
            .is_some(),
        "what was decided before the stop stays decided"
    );
}

#[test]
fn a_canonical_artifact_gone_from_the_store_stops_the_gate() {
    let scratch = Scratch::new();
    scratch.pages(&[("clean", CLEAN)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    let digest = revision_of(&database, &scopes, CLEAN).canonical_digest;
    let digest = digest.as_str();
    let artifact = scratch
        .path()
        .join("kernel")
        .join("artifacts")
        .join("sha256")
        .join(&digest[..2])
        .join(&digest[2..4])
        .join(digest);
    fs::remove_file(artifact).unwrap();
    let refused = quality::gate(&database, &scopes, "garden", &Ledger::default());
    assert!(matches!(refused, Err(Error::Artifacts(_))), "{refused:?}");
}

#[test]
fn every_stop_says_why_and_keeps_its_cause() {
    let not_json = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
    let gone = Digest::of(b"gone");
    let stops = [
        (
            Error::UnknownCollection("orchard".to_owned()),
            "orchard",
            false,
        ),
        (
            Error::Records(document::Error::RevisionConflict("rev-a".to_owned())),
            "rev-a",
            true,
        ),
        (
            Error::Artifacts(store::Error::UnknownArtifact(gone.clone())),
            gone.as_str(),
            true,
        ),
        (
            Error::Canonical {
                revision_id: "rev-b".to_owned(),
                error: not_json,
            },
            "rev-b",
            true,
        ),
    ];
    for (stop, named, caused) in stops {
        let message = stop.to_string();
        assert!(message.contains(named), "{message}");
        assert_eq!(stop.source().is_some(), caused, "{message}");
    }
}
