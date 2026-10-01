//! Lines sharing a `source_ref`: with different digests, each gives its own
//! revision of the one document, and the import holds both, quarantined with
//! its reason and journaled, so neither silently replaces the other, even
//! when one was recorded by an earlier import, and no crash leaves one of
//! them recorded undecided; the same line given twice is one revision, and
//! nothing is held.

use super::support::{
    Scratch, bindings, counts, data_of, declaration, events, everything, import, line, markdown,
    page,
};
use maestro_kernel::{
    artifact::Digest,
    document::{Disposition, Outcome},
    store::Database,
};
use maestro_knowledge::import::{self, Error};
use serde_json::{Value, json};
use std::slice;

/// The type of the event of a revision held back.
const HELD: &str = "maestro.knowledge.revision.held.v1";

/// The data of the event of the revision `revision` of `garden` quarantined.
fn quarantined(revision: &str) -> Value {
    json!({ "collection": "garden", "revision": revision, "disposition": "quarantined" })
}

/// The original digests of the revisions of `garden`, in the order they
/// were recorded.
fn originals(database: &Database) -> Vec<Digest> {
    database
        .eligible_revisions(&everything(database), "garden")
        .unwrap()
        .into_iter()
        .map(|revision| revision.original_digest)
        .collect()
}

#[test]
fn two_lines_sharing_a_source_ref_with_different_digests_are_both_held() {
    let scratch = Scratch::new();
    let [older, newer, seeds] = ["pruning, 2025", "pruning, 2026", "seeds"].map(markdown);
    scratch.put("pruning-2025.md", &older);
    scratch.put("seeds.md", &seeds);
    scratch.put("pruning.md", &newer);
    scratch.manifest(&[
        line("pruning-2025.md", &older, &page("pruning")),
        line("seeds.md", &seeds, &page("seeds")),
        line("pruning.md", &newer, &page("pruning")),
    ]);
    let database = scratch.database();
    assert_eq!(counts(&import(&scratch, &database, "garden")), [1, 0, 2, 0]);
    assert_eq!(
        originals(&database),
        [Digest::of(&older), Digest::of(&seeds), Digest::of(&newer)],
        "both revisions are recorded, neither replacing the other"
    );
    let scopes = everything(&database);
    let revisions = database.eligible_revisions(&scopes, "garden").unwrap();
    let [first, other, second] = revisions.as_slice() else {
        panic!("{revisions:#?}");
    };
    assert_eq!(first.document_id, second.document_id, "one document");
    assert_ne!(first.document_id, other.document_id);
    for held in [first, second] {
        let disposition = database.disposition(&scopes, &held.id).unwrap();
        let Some(Disposition {
            outcome,
            reasons,
            rule_ids,
            decided_by,
            ..
        }) = disposition
        else {
            panic!("{} has no disposition", held.id);
        };
        assert_eq!(outcome, Outcome::Quarantined);
        assert_eq!(decided_by, "import");
        assert_eq!(rule_ids, ["import.shared-source-ref"]);
        let [reason] = reasons.as_slice() else {
            panic!("{reasons:?}");
        };
        assert!(reason.contains(&page("pruning")), "{reason}");
        assert!(reason.contains("different digests"), "{reason}");
    }
    assert_eq!(database.disposition(&scopes, &other.id).unwrap(), None);
    let holds = [first, second].map(|revision| quarantined(&revision.id));
    assert_eq!(data_of(&events(&database, "garden"), HELD), holds);
}

#[test]
fn a_line_given_twice_as_it_is_is_one_revision_and_nothing_is_held() {
    let scratch = Scratch::new();
    let pruning = markdown("pruning");
    scratch.put("pruning.md", &pruning);
    let given = line("pruning.md", &pruning, &page("pruning"));
    scratch.manifest(&[given.clone(), given]);
    let database = scratch.database();
    assert_eq!(counts(&import(&scratch, &database, "garden")), [1, 1, 0, 0]);
    let scopes = everything(&database);
    let revisions = database.eligible_revisions(&scopes, "garden").unwrap();
    let [revision] = revisions.as_slice() else {
        panic!("{revisions:#?}");
    };
    assert_eq!(database.disposition(&scopes, &revision.id).unwrap(), None);
}

#[test]
fn a_line_whose_source_ref_becomes_shared_is_held_by_a_later_import() {
    let scratch = Scratch::new();
    let [older, newer] = ["pruning, 2025", "pruning, 2026"].map(markdown);
    scratch.put("pruning-2025.md", &older);
    scratch.put("pruning.md", &newer);
    let first = line("pruning-2025.md", &older, &page("pruning"));
    scratch.manifest(slice::from_ref(&first));
    let database = scratch.database();
    assert_eq!(counts(&import(&scratch, &database, "garden")), [1, 0, 0, 0]);
    let scopes = everything(&database);
    let recorded = database.eligible_revisions(&scopes, "garden").unwrap();
    let [alone] = recorded.as_slice() else {
        panic!("{recorded:#?}");
    };
    assert_eq!(database.disposition(&scopes, &alone.id).unwrap(), None);
    scratch.manifest(&[first, line("pruning.md", &newer, &page("pruning"))]);
    assert_eq!(counts(&import(&scratch, &database, "garden")), [0, 0, 2, 0]);
    let revisions = database.eligible_revisions(&scopes, "garden").unwrap();
    let ids: Vec<&str> = revisions
        .iter()
        .map(|revision| revision.id.as_str())
        .collect();
    assert_eq!(ids.first(), Some(&alone.id.as_str()), "recorded once");
    for id in &ids {
        let disposition = database.disposition(&scopes, id).unwrap().unwrap();
        assert_eq!(disposition.outcome, Outcome::Quarantined, "{id}");
        assert_eq!(disposition.decided_by, "import", "{id}");
    }
    let holds: Vec<Value> = ids.iter().map(|id| quarantined(id)).collect();
    assert_eq!(data_of(&events(&database, "garden"), HELD), holds);
    assert_eq!(counts(&import(&scratch, &database, "garden")), [0, 2, 0, 0]);
}

#[test]
fn a_pair_whose_hold_is_refused_leaves_no_revision_of_it_recorded() {
    let scratch = Scratch::new();
    let [seeds, older, newer] = ["seeds", "pruning, 2025", "pruning, 2026"].map(markdown);
    scratch.put("seeds.md", &seeds);
    scratch.put("pruning-2025.md", &older);
    scratch.put("pruning.md", &newer);
    scratch.manifest(&[
        line("seeds.md", &seeds, &page("seeds")),
        line("pruning-2025.md", &older, &page("pruning")),
        line("pruning.md", &newer, &page("pruning")),
    ]);
    let database = scratch.database();
    let outside = scratch.outside();
    outside
        .execute_batch(
            "CREATE TRIGGER refuse_dispositions BEFORE INSERT ON quality_dispositions
             BEGIN SELECT RAISE(ABORT, 'the test refuses every disposition'); END;",
        )
        .unwrap();
    let scopes = everything(&database);
    let stopped = import::import(
        &database,
        &scopes,
        &declaration("garden"),
        &bindings(&scratch),
    );
    assert!(matches!(stopped, Err(Error::Records(_))), "{stopped:?}");
    assert_eq!(
        originals(&database),
        [Digest::of(&seeds)],
        "no revision of the pair is recorded undecided"
    );
    outside
        .execute_batch("DROP TRIGGER refuse_dispositions")
        .unwrap();
    assert_eq!(counts(&import(&scratch, &database, "garden")), [0, 1, 2, 0]);
    assert_eq!(
        originals(&database),
        [Digest::of(&seeds), Digest::of(&older), Digest::of(&newer)]
    );
}
