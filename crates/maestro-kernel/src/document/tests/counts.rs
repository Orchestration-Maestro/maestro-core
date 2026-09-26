//! A collection's counts: its documents, and its revisions by status and by
//! quality disposition, read in one snapshot and only inside the caller's
//! scopes.

use super::support::{Scratch, collection, document, revision, source};
use crate::{
    document::{Counts, Disposition, Document, Outcome, Revision, RevisionStatus},
    scope::{Right, ScopeSet},
    store::Database,
};

/// The revision `id` of the document `document_id` with `status`.
fn revision_of(
    database: &Database,
    id: &str,
    document_id: &str,
    status: RevisionStatus,
) -> Revision {
    Revision {
        document_id: document_id.to_owned(),
        ..revision(database, id, status)
    }
}

/// The document `id` of the source `source` of `collection`.
fn document_in(collection: &str, source: &str, id: &str) -> Document {
    Document {
        collection_id: collection.to_owned(),
        source_id: source.to_owned(),
        ..document(id, &format!("https://example.org/{id}"))
    }
}

/// Records `outcome` for the revision `revision_id`, as the quality gate
/// decides it.
fn decide(database: &Database, revision_id: &str, outcome: Outcome) {
    let disposition = Disposition {
        revision_id: revision_id.to_owned(),
        outcome,
        reasons: vec!["checked".to_owned()],
        rule_ids: vec!["quality.test".to_owned()],
        decided_by: "quality".to_owned(),
    };
    database.record_disposition(&disposition).unwrap();
}

/// The database of `scratch`: `ctm` holds `doc-a` and `doc-b` in its source
/// `docs` and `doc-w` in its source `web`, and `other` holds `doc-o`. The
/// revisions of `ctm`'s documents cover every status, and their
/// dispositions some outcomes, one revision left undecided; `other`'s one
/// revision is accepted.
fn recorded(scratch: &Scratch) -> Database {
    let database = scratch.open();
    database.record_source(&source("ctm", "web")).unwrap();
    database.record_collection(&collection("other")).unwrap();
    database.record_source(&source("other", "docs")).unwrap();
    for (collection, source, id) in [
        ("ctm", "docs", "doc-b"),
        ("ctm", "web", "doc-w"),
        ("other", "docs", "doc-o"),
    ] {
        database
            .record_document(&document_in(collection, source, id))
            .unwrap();
    }
    let revisions = [
        (
            "rev-a1",
            "doc-a",
            RevisionStatus::Valid,
            Some(Outcome::Accepted),
        ),
        (
            "rev-a2",
            "doc-a",
            RevisionStatus::ValidWithWarnings,
            Some(Outcome::Quarantined),
        ),
        ("rev-b1", "doc-b", RevisionStatus::Failed, None),
        (
            "rev-b2",
            "doc-b",
            RevisionStatus::Valid,
            Some(Outcome::Accepted),
        ),
        (
            "rev-w1",
            "doc-w",
            RevisionStatus::Valid,
            Some(Outcome::Excluded),
        ),
        (
            "rev-o1",
            "doc-o",
            RevisionStatus::Valid,
            Some(Outcome::Accepted),
        ),
    ];
    for (id, document_id, status, outcome) in revisions {
        let given = revision_of(&database, id, document_id, status);
        database.record_revision(&given).unwrap();
        if let Some(outcome) = outcome {
            decide(&database, id, outcome);
        }
    }
    database
}

/// The counts of `documents` documents and revisions with the statuses and
/// outcomes given in their order, `undecided` of them without a disposition.
fn counts(documents: u64, statuses: [u64; 3], outcomes: [u64; 5], undecided: u64) -> Counts {
    Counts {
        documents,
        statuses: RevisionStatus::ALL.into_iter().zip(statuses).collect(),
        outcomes: Outcome::ALL.into_iter().zip(outcomes).collect(),
        undecided,
    }
}

/// What the principal granted only `scopes` reads.
fn granted(database: &Database, principal: &str, scopes: &[&str]) -> ScopeSet {
    for scope in scopes {
        let scope = scope.parse().unwrap();
        database
            .grant(principal, &scope, Right::Read, "test")
            .unwrap();
    }
    database.visible(principal).unwrap()
}

#[test]
fn a_collection_counts_its_documents_and_its_revisions_by_status_and_disposition() {
    let scratch = Scratch::new();
    let database = recorded(&scratch);
    let everything = ScopeSet::default_workspace();
    assert_eq!(
        database.collection_counts(&everything, "ctm").unwrap(),
        counts(3, [3, 1, 1], [2, 0, 0, 1, 1], 1)
    );
    assert_eq!(
        database.collection_counts(&everything, "other").unwrap(),
        counts(1, [1, 0, 0], [1, 0, 0, 0, 0], 0)
    );
}

#[test]
fn a_collection_nothing_is_recorded_for_counts_zero_of_everything() {
    let scratch = Scratch::new();
    let database = recorded(&scratch);
    assert_eq!(
        database
            .collection_counts(&ScopeSet::default_workspace(), "unknown")
            .unwrap(),
        counts(0, [0; 3], [0; 5], 0)
    );
}

#[test]
fn counts_hold_only_the_records_of_sources_the_scopes_cover() {
    let scratch = Scratch::new();
    let database = recorded(&scratch);
    let docs = granted(
        &database,
        "docs-reader",
        &["workspace/default/collection/ctm/source/docs"],
    );
    assert_eq!(
        database.collection_counts(&docs, "ctm").unwrap(),
        counts(2, [2, 1, 1], [2, 0, 0, 1, 0], 1)
    );
    let web = granted(
        &database,
        "web-reader",
        &["workspace/default/collection/ctm/source/web"],
    );
    assert_eq!(
        database.collection_counts(&web, "ctm").unwrap(),
        counts(1, [1, 0, 0], [0, 0, 0, 0, 1], 0)
    );
    let nothing = database.visible("stranger").unwrap();
    assert_eq!(
        database.collection_counts(&nothing, "ctm").unwrap(),
        counts(0, [0; 3], [0; 5], 0)
    );
}

#[test]
fn statuses_and_outcomes_are_named_as_their_columns_hold_them() {
    let names: Vec<String> = RevisionStatus::ALL
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(names, ["valid", "valid_with_warnings", "failed"]);
    let names: Vec<String> = Outcome::ALL.iter().map(ToString::to_string).collect();
    assert_eq!(
        names,
        [
            "accepted",
            "accepted_with_warnings",
            "needs_reextraction",
            "quarantined",
            "excluded"
        ]
    );
}
