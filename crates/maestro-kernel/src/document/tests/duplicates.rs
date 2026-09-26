//! The duplicates of revisions (01 §6): every place a revision's content
//! occurs, and the groups of near duplicates with their confirmed Jaccard,
//! each recorded once, in one write, and read only where the caller's scopes
//! cover them.

use super::support::{Scratch, revision, source};
use crate::{
    document::{Document, Error, NearDuplicate, Occurrence, RevisionStatus},
    scope::{Right, ScopeSet},
    store::Database,
};

/// The database of `scratch` with the revisions `rev-a` and `rev-b` of
/// `doc-a`, in the source `docs` of `ctm`, and `rev-m` of `doc-m`, in its
/// source `manuals`.
fn opened(scratch: &Scratch) -> Database {
    let database = scratch.open();
    for id in ["rev-a", "rev-b"] {
        database
            .record_revision(&revision(&database, id, RevisionStatus::Valid))
            .unwrap();
    }
    database.record_source(&source("ctm", "manuals")).unwrap();
    database
        .record_document(&Document {
            id: "doc-m".to_owned(),
            collection_id: "ctm".to_owned(),
            source_id: "manuals".to_owned(),
            source_ref: "https://example.org/manual".to_owned(),
        })
        .unwrap();
    let mut manual = revision(&database, "rev-m", RevisionStatus::Valid);
    manual.document_id = "doc-m".to_owned();
    database.record_revision(&manual).unwrap();
    database
}

/// Where the content of `revision` occurs: in the source `source` of `ctm`,
/// from `source_ref`.
fn occurrence(revision: &str, source: &str, source_ref: &str) -> Occurrence {
    Occurrence {
        revision_id: revision.to_owned(),
        collection_id: "ctm".to_owned(),
        source_id: source.to_owned(),
        source_ref: source_ref.to_owned(),
    }
}

/// The member `revision` of the group `group`, with its confirmed Jaccard.
fn member(group: &str, revision: &str, jaccard: f64) -> NearDuplicate {
    NearDuplicate {
        group_id: group.to_owned(),
        revision_id: revision.to_owned(),
        jaccard,
    }
}

/// The scopes of a principal granted the source `source` of `ctm` alone.
fn reading(database: &Database, source: &str) -> ScopeSet {
    let scope = format!("workspace/default/collection/ctm/source/{source}")
        .parse()
        .unwrap();
    database.grant(source, &scope, Right::Read, "test").unwrap();
    database.visible(source).unwrap()
}

#[test]
fn every_occurrence_of_a_revisions_content_is_recorded_once_and_read_back() {
    let scratch = Scratch::new();
    let database = opened(&scratch);
    let occurrences = [
        occurrence("rev-a", "docs", "https://example.org/a"),
        occurrence("rev-a", "manuals", "https://example.org/copy-of-a"),
        occurrence("rev-b", "docs", "https://example.org/b"),
    ];
    database.record_occurrences(&occurrences).unwrap();
    database.record_occurrences(&occurrences).unwrap();
    let everything = ScopeSet::default_workspace();
    // In source and source reference order.
    assert_eq!(
        database.occurrences(&everything, "rev-a").unwrap(),
        occurrences[..2]
    );
    assert_eq!(
        database.occurrences(&everything, "rev-b").unwrap(),
        occurrences[2..]
    );
    assert_eq!(database.occurrences(&everything, "rev-m").unwrap(), []);
}

#[test]
fn an_occurrence_is_read_only_inside_the_scopes_that_cover_its_own_source() {
    let scratch = Scratch::new();
    let database = opened(&scratch);
    let occurrences = [
        occurrence("rev-a", "docs", "https://example.org/a"),
        occurrence("rev-a", "manuals", "https://example.org/copy-of-a"),
    ];
    database.record_occurrences(&occurrences).unwrap();
    assert_eq!(
        database
            .occurrences(&reading(&database, "manuals"), "rev-a")
            .unwrap(),
        occurrences[1..]
    );
    assert_eq!(
        database
            .occurrences(&reading(&database, "docs"), "rev-a")
            .unwrap(),
        occurrences[..1]
    );
}

#[test]
fn occurrences_of_an_unknown_revision_or_source_are_refused_whole() {
    let scratch = Scratch::new();
    let database = opened(&scratch);
    for refused in [
        occurrence("rev-x", "docs", "https://example.org/x"),
        occurrence("rev-a", "archives", "https://example.org/x"),
    ] {
        let batch = [
            occurrence("rev-a", "docs", "https://example.org/a"),
            refused,
        ];
        assert!(matches!(
            database.record_occurrences(&batch),
            Err(Error::Store(_))
        ));
    }
    assert_eq!(
        database
            .occurrences(&ScopeSet::default_workspace(), "rev-a")
            .unwrap(),
        []
    );
}

#[test]
fn a_near_duplicate_group_is_recorded_once_and_read_whole_from_any_member() {
    let scratch = Scratch::new();
    let database = opened(&scratch);
    let group = [
        member("near-1", "rev-a", 0.9),
        member("near-1", "rev-b", 0.875),
        member("near-1", "rev-m", 0.875),
    ];
    database.record_near_duplicates(&group).unwrap();
    database.record_near_duplicates(&group).unwrap();
    let everything = ScopeSet::default_workspace();
    for revision in ["rev-a", "rev-b", "rev-m"] {
        assert_eq!(
            database.near_duplicates(&everything, revision).unwrap(),
            group
        );
    }
    database
        .record_near_duplicates(&[
            member("near-2", "rev-a", 0.95),
            member("near-2", "rev-m", 0.95),
        ])
        .unwrap();
    // Every group of the revision, in group and revision order.
    assert_eq!(
        database.near_duplicates(&everything, "rev-m").unwrap(),
        [
            group.to_vec(),
            vec![
                member("near-2", "rev-a", 0.95),
                member("near-2", "rev-m", 0.95)
            ]
        ]
        .concat()
    );
}

#[test]
fn a_near_duplicate_is_read_only_inside_the_scopes_that_cover_its_source() {
    let scratch = Scratch::new();
    let database = opened(&scratch);
    let group = [
        member("near-1", "rev-a", 0.9),
        member("near-1", "rev-m", 0.9),
    ];
    database.record_near_duplicates(&group).unwrap();
    // The member the caller cannot read is left out, as is a group reached
    // through it.
    assert_eq!(
        database
            .near_duplicates(&reading(&database, "docs"), "rev-a")
            .unwrap(),
        group[..1]
    );
    assert_eq!(
        database
            .near_duplicates(&reading(&database, "docs"), "rev-m")
            .unwrap(),
        []
    );
}

#[test]
fn a_member_outside_the_bounds_of_a_jaccard_or_of_an_unknown_revision_is_refused_whole() {
    let scratch = Scratch::new();
    let database = opened(&scratch);
    for refused in [
        member("near-1", "rev-b", 1.5),
        member("near-1", "rev-b", -0.1),
        member("near-1", "rev-b", f64::NAN),
        member("near-1", "rev-x", 0.9),
    ] {
        assert!(matches!(
            database.record_near_duplicates(&[member("near-1", "rev-a", 0.9), refused]),
            Err(Error::Store(_))
        ));
    }
    assert_eq!(
        database
            .near_duplicates(&ScopeSet::default_workspace(), "rev-a")
            .unwrap(),
        []
    );
}
