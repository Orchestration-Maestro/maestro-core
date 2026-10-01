//! A document is eligible by its latest revision alone, in record order: the
//! page is imported and gated, rewritten at the same `source_ref`, then
//! imported and gated again. An older revision never stands in for a newer
//! one that is held, failed or failed and accepted all the same.

use super::support::{CLEAN, EMPTY, FAILED, Scratch, disposition_of, revision_of};
use maestro_kernel::{
    document::{Disposition, Outcome, Recorded, Revision},
    scope::ScopeSet,
    store::Database,
};
use maestro_knowledge::quality::{self, Ledger};

/// The page rewritten with other bytes, which no check flags.
const REWRITTEN: &str = "# Backups\n\n\
    The backup runs every night at three in the morning, once the export ends.\n";

/// Another page, never rewritten, which no check flags.
const OTHER: &str = "# Restores\n\nA restore reads the latest backup and replays its journal.\n";

/// A kernel in which `page.md` was imported and gated as [`CLEAN`], beside
/// `other.md`, then rewritten as `second` and imported again: the kernel, the
/// scratch directory that holds it, and what its principal reads.
fn rewritten(second: &str) -> (Scratch, Database, ScopeSet) {
    let scratch = Scratch::new();
    scratch.pages(&[("page", CLEAN), ("other", OTHER)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    gate(&database, &scopes);
    assert_eq!(
        disposition_of(&database, &scopes, CLEAN).outcome,
        Outcome::Accepted,
        "the first revision is accepted, so it could stand in"
    );
    scratch.pages(&[("page", second), ("other", OTHER)]);
    scratch.import(&database);
    let [first, latest] = [CLEAN, second].map(|page| revision_of(&database, &scopes, page));
    assert_eq!(first.document_id, latest.document_id, "one document");
    (scratch, database, scopes)
}

/// Runs the gate over `garden` with an empty ledger.
fn gate(database: &Database, scopes: &ScopeSet) {
    quality::gate(database, scopes, "garden", &Ledger::default()).unwrap();
}

/// The eligible revisions of `garden`.
fn eligible(database: &Database, scopes: &ScopeSet) -> Vec<Revision> {
    quality::eligible(database, scopes, "garden").unwrap()
}

#[test]
fn a_document_is_eligible_by_its_latest_revision_alone_in_record_order() {
    let (_scratch, database, scopes) = rewritten(REWRITTEN);
    gate(&database, &scopes);
    assert_eq!(
        eligible(&database, &scopes),
        [OTHER, REWRITTEN].map(|page| revision_of(&database, &scopes, page)),
        "the page's first revision is out, and the other page, recorded before the \
         rewrite, comes first"
    );
}

#[test]
fn a_document_whose_latest_revision_is_held_is_not_eligible() {
    let (_scratch, database, scopes) = rewritten(EMPTY);
    gate(&database, &scopes);
    assert_eq!(
        disposition_of(&database, &scopes, EMPTY).outcome,
        Outcome::NeedsReextraction
    );
    assert_eq!(
        eligible(&database, &scopes),
        [revision_of(&database, &scopes, OTHER)]
    );
}

#[test]
fn a_document_whose_latest_revision_failed_is_not_eligible() {
    let (_scratch, database, scopes) = rewritten(FAILED);
    gate(&database, &scopes);
    assert_eq!(
        eligible(&database, &scopes),
        [revision_of(&database, &scopes, OTHER)]
    );
}

#[test]
fn a_failed_latest_revision_accepted_by_hand_is_not_eligible() {
    let (_scratch, database, scopes) = rewritten(FAILED);
    let failed = revision_of(&database, &scopes, FAILED);
    let by_hand = Disposition {
        revision_id: failed.id.clone(),
        outcome: Outcome::Accepted,
        reasons: vec!["accepted by hand, whatever canonicalization says".to_owned()],
        rule_ids: vec!["test.by-hand".to_owned()],
        decided_by: "tester".to_owned(),
    };
    assert_eq!(
        database.record_disposition(&by_hand).unwrap(),
        Recorded::New
    );
    gate(&database, &scopes);
    assert_eq!(disposition_of(&database, &scopes, FAILED), by_hand);
    assert_eq!(
        eligible(&database, &scopes),
        [revision_of(&database, &scopes, OTHER)]
    );
}
