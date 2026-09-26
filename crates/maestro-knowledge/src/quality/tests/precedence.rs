//! The precedence of a decision: a ledger rule that matches outranks every
//! automatic check, but never accepts a failed canonical document; among the
//! checks, the most severe outcome decides, and every check that flags the
//! revision is named.

use super::super::decide::{GATE, decide};
use super::support::{OUTCOMES, PAGE, candidate, document, guide, ledger, revision};
use maestro_kernel::document::{Disposition, Outcome};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// A page whose body is a single link: near-empty.
const EMPTY: &str = "# Garden party\n\n[See every event](https://example.org/events)\n";
/// A page that no check flags.
const CLEAN: &str = "# Backups\n\nThe backup runs every night at two in the morning.\n";
/// A page whose front matter contradicts its title: canonicalization fails.
const FAILED: &str = "---\ntitle: Another page\n---\n# Page\n\n\
    What the page says about the backups of the database.\n";

/// The decision on the page `markdown`, a guide of the set `notes`, with the
/// ledger of `rules`.
fn decided(markdown: &str, rules: &[Value]) -> Disposition {
    let document = document(markdown);
    let revision = revision(&document, &guide());
    decide(&ledger(rules), &candidate(&revision), &document, markdown)
}

#[test]
fn a_ledger_rule_outranks_every_automatic_check() {
    let exception = json!({
        "id": "keep-the-party", "match": { "source_ref": PAGE }, "disposition": "accepted",
        "reason": "the invitation is all the page is meant to hold", "decided_by": "Ada",
    });
    let decision = decided(EMPTY, &[exception]);
    assert_eq!(decision.outcome, Outcome::Accepted);
    assert_eq!(decision.rule_ids, ["ledger.keep-the-party"]);
    assert_eq!(
        decision.reasons,
        ["the invitation is all the page is meant to hold"]
    );
    assert_eq!(decision.decided_by, "Ada");
}

#[test]
fn a_ledger_rule_holds_back_a_revision_the_checks_accept() {
    let retired =
        json!({ "id": "retired", "match": { "set": "notes" }, "disposition": "excluded" });
    let decision = decided(CLEAN, &[retired]);
    assert_eq!(decision.outcome, Outcome::Excluded);
    assert_eq!(decision.rule_ids, ["ledger.retired"]);
    assert_eq!(decision.decided_by, "owner");
}

#[test]
fn the_first_rule_that_matches_decides() {
    let rules = [
        json!({ "id": "other-set", "match": { "set": "seeds" }, "disposition": "excluded" }),
        json!({ "id": "first", "match": { "version": "1.0" }, "disposition": "quarantined" }),
        json!({ "id": "second", "match": { "set": "notes" }, "disposition": "excluded" }),
    ];
    let decision = decided(CLEAN, &rules);
    assert_eq!(decision.outcome, Outcome::Quarantined);
    assert_eq!(decision.rule_ids, ["ledger.first"]);
}

#[test]
fn a_rule_matches_when_every_field_it_names_is_equal() {
    let document = document(CLEAN);
    let revision = revision(&document, &guide());
    let sha256 = revision.original_digest.as_str().to_owned();
    for (fields, matches) in [
        (json!({ "set": "notes", "version": "1.0" }), true),
        (json!({ "set": "notes", "version": "2.0" }), false),
        (json!({ "title": "Page", "source_kind": "guide" }), true),
        (json!({ "source_kind": "Guide" }), false),
        (json!({ "sha256": sha256 }), true),
        (json!({ "lang": "en" }), false),
        (json!({ "source_ref": "https://example.org/other" }), false),
    ] {
        let rule = json!({ "id": "rule", "match": fields, "disposition": "excluded" });
        let decision = decide(&ledger(&[rule]), &candidate(&revision), &document, CLEAN);
        assert_eq!(decision.outcome == Outcome::Excluded, matches, "{fields}");
    }
}

#[test]
fn a_rule_never_accepts_a_failed_canonical_document() {
    for disposition in ["accepted", "accepted_with_warnings"] {
        let rule = json!({ "id": "keep", "match": { "set": "notes" }, "disposition": disposition });
        let decision = decided(FAILED, &[rule]);
        assert_eq!(
            decision.outcome,
            Outcome::NeedsReextraction,
            "{disposition}"
        );
        assert_eq!(decision.rule_ids, ["canonicalization.failed"]);
        assert_eq!(decision.decided_by, GATE);
        let [failure, set_aside] = decision.reasons.as_slice() else {
            panic!("{decision:?}");
        };
        assert!(failure.starts_with("canonicalization failed"), "{failure}");
        assert!(
            set_aside.contains("ledger.keep") && set_aside.contains("never indexed"),
            "{set_aside}"
        );
    }
}

#[test]
fn a_rule_that_holds_a_failed_canonical_document_back_decides_it() {
    let rule = json!({ "id": "retired", "match": { "set": "notes" }, "disposition": "excluded" });
    let decision = decided(FAILED, &[rule]);
    assert_eq!(decision.outcome, Outcome::Excluded);
    assert_eq!(decision.rule_ids, ["ledger.retired"]);
}

#[test]
fn the_most_severe_check_decides_and_every_check_that_flags_is_named() {
    // A warning, one replacement character in a long link label, and a hold:
    // the body is that link alone.
    let decision = decided(
        "# Garden party\n\n[See every event of the garden club this summer, from the \
         first seed swap in May to the harvest supper in September, and \u{fffd}more]\
         (https://example.org/events)\n",
        &[],
    );
    assert_eq!(decision.outcome, Outcome::NeedsReextraction);
    assert_eq!(
        decision.rule_ids,
        ["body.near-empty", "text.replacement-characters"]
    );
    assert!(
        decision.reasons[1].starts_with("1 replacement"),
        "{decision:?}"
    );
    assert_eq!(decision.decided_by, GATE);
    let unknown = super::support::canonical(EMPTY, None, Some("Party"), BTreeMap::new());
    let revision = revision(&unknown, &guide());
    let decision = decide(&ledger(&[]), &candidate(&revision), &unknown, EMPTY);
    assert_eq!(decision.outcome, Outcome::Quarantined);
    assert_eq!(decision.rule_ids, ["metadata.missing", "body.near-empty"]);
}

#[test]
fn a_revision_no_check_flags_is_accepted() {
    let decision = decided(CLEAN, &[]);
    assert_eq!(
        decision,
        Disposition {
            revision_id: decision.revision_id.clone(),
            outcome: Outcome::Accepted,
            reasons: Vec::new(),
            rule_ids: Vec::new(),
            decided_by: GATE.to_owned(),
        }
    );
    assert!(decision.revision_id.starts_with("rev-"));
}

#[test]
fn the_outcomes_hold_back_from_needs_reextraction_on() {
    let holds: Vec<bool> = OUTCOMES
        .into_iter()
        .map(super::super::outcome::holds)
        .collect();
    assert_eq!(holds, [false, false, true, true, true]);
    let names: Vec<&str> = OUTCOMES
        .into_iter()
        .map(super::super::outcome::name)
        .collect();
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
