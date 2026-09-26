//! A disposition, once recorded, is kept: a rerun of the gate decides
//! nothing again, and a revision the import held keeps its quarantine even
//! when a ledger rule would accept it; the report counts each ledger rule a
//! kept disposition outranks. Otherwise a ledger rule outranks the automatic
//! checks.

use super::support::{CLEAN, EMPTY, HELD, Scratch, data_of, disposition_of, events, revision_of};
use maestro_kernel::document::Outcome;
use maestro_knowledge::quality::{self, Ledger};
use serde_json::json;
use std::collections::BTreeMap;

/// The ledger of `rules`, one JSON rule a line.
fn ledger(rules: &[serde_json::Value]) -> Ledger {
    rules
        .iter()
        .map(|rule| format!("{rule}\n"))
        .collect::<Vec<_>>()
        .concat()
        .parse()
        .unwrap()
}

/// A rule of Ada's, `id`, that gives the page `name` the `disposition`.
fn decision(id: &str, name: &str, disposition: &str) -> serde_json::Value {
    json!({
        "schema": "maestro-quality-ledger/1",
        "id": id,
        "match": { "source_ref": format!("https://example.org/{name}") },
        "disposition": disposition,
        "reason": format!("Ada reviewed {name}"),
        "decided_by": "Ada",
        "reversal": "remove this rule before the page is decided",
    })
}

#[test]
fn a_rerun_keeps_every_disposition_and_records_nothing() {
    let scratch = Scratch::new();
    scratch.pages(&[("clean", CLEAN), ("empty", EMPTY)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    let first = quality::gate(&database, &scopes, "garden", &Ledger::default()).unwrap();
    let recorded = [CLEAN, EMPTY].map(|page| disposition_of(&database, &scopes, page));
    let journaled = events(&database, &scopes).len();
    let accepting = ledger(&[decision("keep", "empty", "accepted")]);
    let second = quality::gate(&database, &scopes, "garden", &accepting).unwrap();
    assert_eq!([first.decided, first.kept], [2, 0]);
    assert_eq!([second.decided, second.kept], [0, 2]);
    assert_eq!(second.outcomes, first.outcomes);
    assert_eq!(second.rules, first.rules);
    assert_eq!(second.held, first.held);
    assert_eq!(
        [CLEAN, EMPTY].map(|page| disposition_of(&database, &scopes, page)),
        recorded,
        "a rule written since decides no revision decided before it"
    );
    assert_eq!(events(&database, &scopes).len(), journaled);
}

#[test]
fn a_revision_the_import_held_keeps_its_quarantine() {
    let scratch = Scratch::new();
    let pruning = "https://example.org/pruning".to_owned();
    scratch.lines(&[
        (
            "pruning-2025.md".to_owned(),
            pruning.clone(),
            "# Pruning\n\nCut in March.\n",
        ),
        (
            "pruning.md".to_owned(),
            pruning,
            "# Pruning\n\nCut in February, before the sap rises.\n",
        ),
        (
            "clean.md".to_owned(),
            "https://example.org/clean".to_owned(),
            CLEAN,
        ),
    ]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    let held_by_import = data_of(&events(&database, &scopes), HELD);
    let report = quality::gate(
        &database,
        &scopes,
        "garden",
        &ledger(&[decision("keep", "pruning", "accepted")]),
    )
    .unwrap();
    assert_eq!([report.revisions, report.decided, report.kept], [3, 1, 2]);
    assert_eq!(report.outcomes.quarantined, 2);
    assert_eq!(
        report.ignored_rules,
        BTreeMap::from([("ledger.keep".to_owned(), 2)])
    );
    assert_eq!(report.rules.get("import.shared-source-ref"), Some(&2));
    let decided_by: Vec<(&str, Outcome)> = report
        .held
        .iter()
        .map(|held| (held.decided_by.as_str(), held.outcome))
        .collect();
    assert_eq!(
        decided_by,
        [
            ("import", Outcome::Quarantined),
            ("import", Outcome::Quarantined)
        ]
    );
    assert_eq!(
        data_of(&events(&database, &scopes), HELD),
        held_by_import,
        "the gate held nothing more"
    );
}

#[test]
fn a_first_matching_rule_a_kept_disposition_outranks_is_counted_as_ignored() {
    let scratch = Scratch::new();
    scratch.pages(&[("clean", CLEAN), ("empty", EMPTY)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    let first = quality::gate(&database, &scopes, "garden", &Ledger::default()).unwrap();
    assert_eq!(first.ignored_rules, BTreeMap::new());
    // `empty` keeps needs_reextraction against `keep`; `clean`, accepted
    // already, loses nothing to `confirm`, and `later` is not its first rule.
    let rules = ledger(&[
        decision("keep", "empty", "accepted"),
        decision("confirm", "clean", "accepted"),
        decision("later", "clean", "excluded"),
    ]);
    let second = quality::gate(&database, &scopes, "garden", &rules).unwrap();
    assert_eq!([second.decided, second.kept], [0, 2]);
    assert_eq!(
        second.ignored_rules,
        BTreeMap::from([("ledger.keep".to_owned(), 1)])
    );
    assert_eq!(
        disposition_of(&database, &scopes, EMPTY).outcome,
        Outcome::NeedsReextraction
    );
}

#[test]
fn a_ledger_rule_decides_before_the_automatic_checks() {
    let scratch = Scratch::new();
    scratch.pages(&[("empty", EMPTY), ("clean", CLEAN)]);
    let database = scratch.database();
    let scopes = scratch.import(&database);
    let rules = ledger(&[
        decision("keep-the-party", "empty", "accepted"),
        decision("retire-backups", "clean", "excluded"),
    ]);
    quality::gate(&database, &scopes, "garden", &rules).unwrap();
    let kept = disposition_of(&database, &scopes, EMPTY);
    assert_eq!(kept.outcome, Outcome::Accepted);
    assert_eq!(kept.rule_ids, ["ledger.keep-the-party"]);
    assert_eq!(kept.reasons, ["Ada reviewed empty"]);
    assert_eq!(kept.decided_by, "Ada");
    let retired = disposition_of(&database, &scopes, CLEAN);
    assert_eq!(retired.outcome, Outcome::Excluded);
    assert_eq!(retired.rule_ids, ["ledger.retire-backups"]);
    assert_eq!(
        data_of(&events(&database, &scopes), HELD),
        [json!({
            "collection": "garden",
            "revision": revision_of(&database, &scopes, CLEAN).id,
            "disposition": "excluded",
        })]
    );
}
