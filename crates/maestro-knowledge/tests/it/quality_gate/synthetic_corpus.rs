//! The public synthetic collection, `tests/fixtures/synthetic` (T014),
//! imported and gated as its declaration names it: its handbook of guides,
//! runbooks, references and a glossary in two languages is content every
//! check lets through, and its declaration names a ledger it does not hold.

use super::support::{GATE, Scratch};
use maestro_kernel::{binding::Bindings, scope::Right};
use maestro_knowledge::{
    collection::Declaration,
    import,
    quality::{self, Ledger},
};
use std::{fs, path::Path};

#[test]
fn the_synthetic_collection_passes_the_gate_with_the_ledger_it_does_not_hold() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("tests")
        .join("fixtures")
        .join("synthetic");
    let declaration: Declaration = fs::read_to_string(fixture.join("collection.json"))
        .unwrap()
        .parse()
        .unwrap();
    let ledger = Ledger::load(&declaration.quality.ledger.under(&fixture)).unwrap();
    assert_eq!(ledger, Ledger::default(), "no ledger file: an empty ledger");
    let bindings: Bindings = format!("synthetic_root = '{}'\n", fixture.display())
        .parse()
        .unwrap();
    let scratch = Scratch::new();
    let database = scratch.database();
    let collection = "workspace/default/collection/synthetic".parse().unwrap();
    database
        .grant("reader", &collection, Right::Read, "test")
        .unwrap();
    let scopes = database.visible("reader").unwrap();
    import::import(&database, &scopes, &declaration, &bindings).unwrap();
    let report = quality::gate(&database, &scopes, "synthetic", &ledger).unwrap();
    assert_eq!(
        [report.revisions, report.decided, report.outcomes.accepted],
        [28, 28, 28],
        "{report:#?}"
    );
    assert!(
        report.held.is_empty() && report.rules.is_empty(),
        "{report:#?}"
    );
    let eligible = quality::eligible(&database, &scopes, "synthetic").unwrap();
    assert_eq!(eligible, database.revisions(&scopes, "synthetic").unwrap());
    let decided_by = database
        .disposition(&scopes, &eligible[0].id)
        .unwrap()
        .unwrap()
        .decided_by;
    assert_eq!(decided_by, GATE);
}
