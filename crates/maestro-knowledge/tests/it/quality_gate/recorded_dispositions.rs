//! What the gate records of each revision (T020 step 1): the disposition its
//! checks give, with their rule IDs and reasons; `revision.held` for each one
//! held back; eligibility for the accepted ones alone; and, with no ledger,
//! the checks alone.

use super::support::{
    CLEAN, EMPTY, FAILED, GATE, HELD, REPLACED, Scratch, data_of, disposition_of, events,
    revision_of,
};
use maestro_kernel::document::{Disposition, Outcome, RevisionStatus};
use maestro_knowledge::quality::{self, Ledger};
use serde_json::json;

/// The four pages the tests import: accepted, near-empty, failed, and
/// accepted with a warning, in that order.
const PAGES: [(&str, &str); 4] = [
    ("clean", CLEAN),
    ("empty", EMPTY),
    ("failed", FAILED),
    ("replaced", REPLACED),
];

#[test]
fn each_revision_receives_the_disposition_its_checks_give_with_their_rule_ids_and_reasons() {
    let scratch = Scratch::new();
    scratch.pages(&PAGES);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    let report = quality::gate(&database, &scopes, "garden", &Ledger::default()).unwrap();
    assert_eq!([report.revisions, report.decided, report.kept], [4, 4, 0]);
    let clean = disposition_of(&database, &scopes, CLEAN);
    assert_eq!(
        clean,
        Disposition {
            revision_id: clean.revision_id.clone(),
            outcome: Outcome::Accepted,
            reasons: Vec::new(),
            rule_ids: Vec::new(),
            decided_by: GATE.to_owned(),
        }
    );
    for (markdown, outcome, rule, reason) in [
        (
            EMPTY,
            Outcome::NeedsReextraction,
            "body.near-empty",
            "fewer than 8",
        ),
        (
            FAILED,
            Outcome::NeedsReextraction,
            "canonicalization.failed",
            "metadata_conflict",
        ),
        (
            REPLACED,
            Outcome::AcceptedWithWarnings,
            "text.replacement-characters",
            "1 replacement character",
        ),
    ] {
        let decided = disposition_of(&database, &scopes, markdown);
        assert_eq!(decided.outcome, outcome, "{decided:?}");
        assert_eq!(decided.rule_ids, [rule]);
        let [given] = decided.reasons.as_slice() else {
            panic!("{decided:?}");
        };
        assert!(given.contains(reason), "{given}");
        assert_eq!(decided.decided_by, GATE);
    }
    assert_eq!(
        revision_of(&database, &scopes, FAILED).status,
        RevisionStatus::Failed,
        "the import recorded it failed, and it is decided all the same"
    );
}

#[test]
fn only_revisions_accepted_with_or_without_warnings_are_eligible() {
    let scratch = Scratch::new();
    scratch.pages(&PAGES);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    assert_eq!(
        quality::eligible(&database, &scopes, "garden").unwrap(),
        Vec::new(),
        "a revision without a disposition is not eligible"
    );
    quality::gate(&database, &scopes, "garden", &Ledger::default()).unwrap();
    assert_eq!(
        quality::eligible(&database, &scopes, "garden").unwrap(),
        [
            revision_of(&database, &scopes, CLEAN),
            revision_of(&database, &scopes, REPLACED)
        ],
        "in record order"
    );
    let nobody = database.visible("nobody").unwrap();
    assert_eq!(
        quality::eligible(&database, &nobody, "garden").unwrap(),
        Vec::new()
    );
}

#[test]
fn each_revision_held_back_is_journaled_as_revision_held() {
    let scratch = Scratch::new();
    scratch.pages(&PAGES);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    assert_eq!(data_of(&events(&database, &scopes), HELD), [json!(null); 0]);
    quality::gate(&database, &scopes, "garden", &Ledger::default()).unwrap();
    let held = [EMPTY, FAILED].map(|markdown| {
        json!({
            "collection": "garden",
            "revision": revision_of(&database, &scopes, markdown).id,
            "disposition": "needs_reextraction",
        })
    });
    assert_eq!(data_of(&events(&database, &scopes), HELD), held);
}

#[test]
fn a_missing_ledger_reads_as_an_empty_one() {
    let scratch = Scratch::new();
    scratch.pages(&[("empty", EMPTY)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    let ledger = Ledger::load(&scratch.path().join("quality").join("ledger.jsonl")).unwrap();
    assert_eq!(ledger, Ledger::default());
    quality::gate(&database, &scopes, "garden", &ledger).unwrap();
    assert_eq!(
        disposition_of(&database, &scopes, EMPTY).rule_ids,
        ["body.near-empty"]
    );
}
