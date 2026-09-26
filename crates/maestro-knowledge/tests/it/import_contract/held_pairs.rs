//! Lines sharing a `source_ref`: with different digests, each gives its own
//! revision of the one document, and the import holds both, quarantined with
//! its reason and journaled, so neither silently replaces the other; the
//! same line given twice is one revision, and nothing is held.

use super::support::{Scratch, data_of, events, everything, import, line, markdown, page};
use maestro_kernel::{
    artifact::Digest,
    document::{Disposition, Outcome},
};
use serde_json::json;

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
    let report = import(&scratch, &database, "garden");
    assert_eq!(
        [
            report.imported,
            report.unchanged,
            report.held,
            report.refused
        ],
        [1, 0, 2, 0]
    );
    let scopes = everything(&database);
    let revisions = database.eligible_revisions(&scopes, "garden").unwrap();
    let originals: Vec<Digest> = revisions
        .iter()
        .map(|revision| revision.original_digest.clone())
        .collect();
    assert_eq!(
        originals,
        [Digest::of(&older), Digest::of(&seeds), Digest::of(&newer)],
        "both revisions are recorded, neither replacing the other"
    );
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
    let holds = [first, second].map(|revision| {
        json!({ "collection": "garden", "revision": revision.id, "disposition": "quarantined" })
    });
    assert_eq!(
        data_of(
            &events(&database, "garden"),
            "maestro.knowledge.revision.held.v1"
        ),
        holds
    );
}

#[test]
fn a_line_given_twice_as_it_is_is_one_revision_and_nothing_is_held() {
    let scratch = Scratch::new();
    let pruning = markdown("pruning");
    scratch.put("pruning.md", &pruning);
    let given = line("pruning.md", &pruning, &page("pruning"));
    scratch.manifest(&[given.clone(), given]);
    let database = scratch.database();
    let report = import(&scratch, &database, "garden");
    assert_eq!(
        [
            report.imported,
            report.unchanged,
            report.held,
            report.refused
        ],
        [1, 1, 0, 0]
    );
    let scopes = everything(&database);
    let revisions = database.eligible_revisions(&scopes, "garden").unwrap();
    let [revision] = revisions.as_slice() else {
        panic!("{revisions:#?}");
    };
    assert_eq!(database.disposition(&scopes, &revision.id).unwrap(), None);
}
