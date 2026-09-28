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
fn batched_near_duplicates_match_the_ordered_union_of_per_revision_reads() {
    let scratch = Scratch::new();
    let database = opened(&scratch);
    database
        .record_near_duplicates(&[
            member("near-1", "rev-a", 0.9),
            member("near-1", "rev-b", 0.875),
            member("near-1", "rev-m", 0.875),
            member("near-2", "rev-a", 0.95),
            member("near-2", "rev-m", 0.95),
        ])
        .unwrap();
    let revisions = ["rev-b", "rev-a", "rev-m", "rev-a", "unknown"];
    let everything = ScopeSet::default_workspace();
    let batched = database
        .near_duplicates_for_revisions(&everything, &revisions.map(str::to_owned))
        .unwrap();
    let mut per_revision = Vec::new();
    for revision in revisions {
        per_revision.extend(database.near_duplicates(&everything, revision).unwrap());
    }
    per_revision.sort_by(|left, right| {
        left.group_id
            .cmp(&right.group_id)
            .then_with(|| left.revision_id.cmp(&right.revision_id))
    });
    per_revision.dedup_by(|left, right| {
        left.group_id == right.group_id && left.revision_id == right.revision_id
    });
    assert_eq!(batched, per_revision);

    let documents = reading(&database, "docs");
    assert_eq!(
        database
            .near_duplicates_for_revisions(&documents, &revisions.map(str::to_owned))
            .unwrap(),
        vec![
            member("near-1", "rev-a", 0.9),
            member("near-1", "rev-b", 0.875),
            member("near-2", "rev-a", 0.95),
        ]
    );
}

#[test]
fn an_existing_near_duplicate_group_requires_its_complete_membership() {
    let expected = [
        member("near-1", "rev-a", 0.9),
        member("near-1", "rev-b", 0.875),
    ];
    let changed = [
        member("near-1", "rev-a", 0.95),
        member("near-1", "rev-b", 0.875),
    ];
    let extra = [
        member("near-1", "rev-a", 0.9),
        member("near-1", "rev-b", 0.875),
        member("near-1", "rev-m", 0.875),
    ];
    let missing = [member("near-1", "rev-a", 0.9)];

    for attempted in [changed.as_slice(), extra.as_slice(), missing.as_slice()] {
        let scratch = Scratch::new();
        let database = opened(&scratch);
        database.record_near_duplicates(&expected).unwrap();
        let error = database.record_near_duplicates(attempted).unwrap_err();
        assert!(
            matches!(error, Error::NearDuplicateConflict(ref group) if group == "near-1"),
            "unexpected error for {attempted:?}: {error}"
        );
        assert_eq!(
            database
                .near_duplicates(&ScopeSet::default_workspace(), "rev-a")
                .unwrap(),
            expected
        );
    }
}

#[test]
fn one_member_cannot_have_two_jaccards_in_one_batch() {
    let scratch = Scratch::new();
    let database = opened(&scratch);
    let conflict = [
        member("near-1", "rev-a", 0.9),
        member("near-1", "rev-a", 0.95),
    ];

    assert!(matches!(
        database.record_near_duplicates(&conflict),
        Err(Error::NearDuplicateConflict(group)) if group == "near-1"
    ));
    assert_eq!(
        database
            .near_duplicates(&ScopeSet::default_workspace(), "rev-a")
            .unwrap(),
        []
    );
}

#[test]
fn a_group_conflict_rolls_back_other_new_groups_in_the_same_call() {
    let scratch = Scratch::new();
    let database = opened(&scratch);
    let existing = [
        member("near-1", "rev-a", 0.9),
        member("near-1", "rev-b", 0.875),
    ];
    database.record_near_duplicates(&existing).unwrap();

    let result = database.record_near_duplicates(&[
        member("near-0-new", "rev-m", 0.4),
        member("near-1", "rev-a", 0.95),
        member("near-1", "rev-b", 0.875),
    ]);

    assert!(result.is_err(), "accepted a changed existing group");
    let all = ScopeSet::default_workspace();
    assert_eq!(database.near_duplicates(&all, "rev-m").unwrap(), []);
    assert_eq!(database.near_duplicates(&all, "rev-a").unwrap(), existing);
}

#[test]
fn direct_sql_writes_are_outside_the_method_level_membership_guard() {
    let scratch = Scratch::new();
    let database = opened(&scratch);
    let expected = [
        member("near-1", "rev-a", 0.9),
        member("near-1", "rev-b", 0.875),
    ];
    database.record_near_duplicates(&expected).unwrap();

    database
        .write(|transaction| {
            transaction.execute(
                "INSERT INTO near_dup_groups (group_id, revision_id, jaccard)
                 VALUES ('near-1', 'rev-m', 0.8)",
                [],
            )?;
            Ok::<_, Error>(())
        })
        .unwrap();

    assert!(matches!(
        database.record_near_duplicates(&expected),
        Err(Error::NearDuplicateConflict(group)) if group == "near-1"
    ));
}

#[test]
fn identical_reordered_group_rows_are_a_no_op() {
    let scratch = Scratch::new();
    let database = opened(&scratch);
    let expected = [
        member("near-1", "rev-a", 0.9),
        member("near-1", "rev-b", 0.875),
    ];
    database.record_near_duplicates(&expected).unwrap();
    database
        .record_near_duplicates(&[
            expected[1].clone(),
            expected[0].clone(),
            expected[0].clone(),
        ])
        .unwrap();
    assert_eq!(
        database
            .near_duplicates(&ScopeSet::default_workspace(), "rev-a")
            .unwrap(),
        expected
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
