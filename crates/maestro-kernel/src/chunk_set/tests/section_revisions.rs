//! Scoped section lookups stop after two distinct document/revision pairs.

use super::support::{Scratch, chunk, new_set, prepared, reading};
use crate::{
    chunk_set::Chunk,
    document::{Disposition, Document, Outcome, Revision, RevisionStatus},
    store::Database,
};
use serde_json::Map;
use std::{collections::BTreeSet, time::Instant};

fn decide(database: &Database, revision_id: &str, outcome: Outcome) {
    database
        .record_disposition(&Disposition {
            revision_id: revision_id.to_owned(),
            outcome,
            reasons: Vec::new(),
            rule_ids: Vec::new(),
            decided_by: "test".to_owned(),
        })
        .unwrap();
}

#[test]
fn returns_at_most_two_distinct_pairs_inside_the_scopes() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database.begin_chunk_set(&new_set("set-a", "ctm")).unwrap();
    let input = prepared(&database, "section source");
    let mut duplicate = chunk("chunk-a2", "rev-a", &input, 1, [0, 1]);
    duplicate.section_id = Some("shared".to_owned());
    let mut first = chunk("chunk-a1", "rev-a", &input, 1, [0, 1]);
    first.section_id = Some("shared".to_owned());
    let mut second = chunk("chunk-b1", "rev-b", &input, 1, [0, 1]);
    second.section_id = Some("shared".to_owned());
    database
        .record_chunks("set-a", "rev-a", &[first, duplicate])
        .unwrap();
    database.record_chunks("set-a", "rev-b", &[second]).unwrap();
    database
        .record_document(&Document {
            id: "doc-c".to_owned(),
            collection_id: "ctm".to_owned(),
            source_id: "docs".to_owned(),
            source_ref: "https://example.org/doc-c".to_owned(),
        })
        .unwrap();
    let original = database.put(b"# rev-c\n", "text/markdown").unwrap();
    let canonical = database.put(b"{}", "application/json").unwrap();
    database
        .record_revision(&Revision {
            id: "rev-c".to_owned(),
            document_id: "doc-c".to_owned(),
            original_digest: original,
            canonical_digest: canonical,
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata: Map::new(),
        })
        .unwrap();
    let mut third = chunk("chunk-c1", "rev-c", &input, 1, [0, 1]);
    third.section_id = Some("shared".to_owned());
    database.record_chunks("set-a", "rev-c", &[third]).unwrap();
    for revision_id in ["rev-a", "rev-b", "rev-c"] {
        decide(&database, revision_id, Outcome::Accepted);
    }

    let found = database
        .section_revisions(&reading(&database, "ctm"), "set-a", "shared")
        .unwrap();
    let allowed = BTreeSet::from([
        ("doc-a".to_owned(), "rev-a".to_owned()),
        ("doc-a".to_owned(), "rev-b".to_owned()),
        ("doc-c".to_owned(), "rev-c".to_owned()),
    ]);
    assert_eq!(found.len(), 2);
    assert_eq!(found.iter().cloned().collect::<BTreeSet<_>>().len(), 2);
    assert!(found.iter().all(|pair| allowed.contains(pair)));
    assert!(
        database
            .section_revisions(&reading(&database, "other"), "set-a", "shared")
            .unwrap()
            .is_empty()
    );
}

#[test]
fn includes_accepted_warnings_and_omits_nonaccepted_dispositions() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database.begin_chunk_set(&new_set("set-a", "ctm")).unwrap();
    let input = prepared(&database, "section source");
    let mut accepted = chunk("chunk-a1", "rev-a", &input, 1, [0, 1]);
    accepted.section_id = Some("shared".to_owned());
    let mut quarantined = chunk("chunk-b1", "rev-b", &input, 1, [0, 1]);
    quarantined.section_id = Some("shared".to_owned());
    database
        .record_chunks("set-a", "rev-a", &[accepted])
        .unwrap();
    database
        .record_chunks("set-a", "rev-b", &[quarantined])
        .unwrap();
    decide(&database, "rev-a", Outcome::AcceptedWithWarnings);
    decide(&database, "rev-b", Outcome::Quarantined);

    assert_eq!(
        database
            .section_revisions(&reading(&database, "ctm"), "set-a", "shared")
            .unwrap(),
        [("doc-a".to_owned(), "rev-a".to_owned())]
    );
}

#[test]
#[ignore = "measure the read path over a 50,000-chunk set"]
fn measure_section_lookup_on_50k_chunks() {
    let scratch = Scratch::new();
    let database = scratch.open();
    database.begin_chunk_set(&new_set("set-a", "ctm")).unwrap();
    let input = prepared(&database, "section source");
    let chunks: Vec<Chunk> = (0..50_000)
        .map(|index| {
            let id = format!("chunk-{index}");
            let mut chunk = chunk(&id, "rev-a", &input, 1, [0, 1]);
            chunk.section_id = Some("selected".to_owned());
            chunk
        })
        .collect();
    database.record_chunks("set-a", "rev-a", &chunks).unwrap();
    decide(&database, "rev-a", Outcome::Accepted);
    let scopes = reading(&database, "ctm");

    let started = Instant::now();
    let found = database
        .section_revisions(&scopes, "set-a", "selected")
        .unwrap();
    let elapsed = started.elapsed();

    assert_eq!(found, [("doc-a".to_owned(), "rev-a".to_owned())]);
    println!("50,000-chunk section lookup: {elapsed:?}");
}
