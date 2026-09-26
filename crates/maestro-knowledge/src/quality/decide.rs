//! One decision of the gate, on a revision no one has decided yet: the
//! ledger's first rule that matches it, else the automatic checks.

use super::{
    checks,
    ledger::{Candidate, Ledger},
    outcome,
};
use maestro_canonicalization::{CanonicalDocument, ValidationStatus};
use maestro_kernel::document::{Disposition, Outcome};
use std::cmp::Reverse;

/// Who the gate names as the decider of what its checks decide, with the
/// version of its rules and thresholds.
pub(super) const GATE: &str = "quality-gate/1";

/// The disposition of `candidate`, whose canonical document is `document`:
/// the first rule of `ledger` that matches it, unless that rule would accept
/// a failed document, which is never indexed; else the most severe outcome
/// of the automatic checks that flag it, each named with its reason, the
/// most severe first, or `accepted` when none does.
pub(super) fn decide(
    ledger: &Ledger,
    candidate: &Candidate<'_>,
    document: &CanonicalDocument,
    markdown: &str,
) -> Disposition {
    let revision_id = candidate.revision().id.clone();
    let mut set_aside = None;
    if let Some(rule) = ledger.first_match(candidate) {
        if outcome::holds(rule.disposition)
            || document.validation_status != ValidationStatus::Failed
        {
            return Disposition {
                revision_id,
                outcome: rule.disposition,
                reasons: vec![rule.reason.clone()],
                rule_ids: vec![rule.rule_id()],
                decided_by: rule.decided_by.clone(),
            };
        }
        set_aside = Some(format!(
            "the ledger rule {} accepts it, but a failed canonical document is never indexed",
            rule.rule_id()
        ));
    }
    let mut flags = checks::run(document, markdown);
    flags.sort_by_key(|flag| Reverse(severity(flag.outcome)));
    let decided = flags.first().map_or(Outcome::Accepted, |flag| flag.outcome);
    let rule_ids = flags.iter().map(|flag| flag.rule.to_owned()).collect();
    let mut reasons: Vec<String> = flags.into_iter().map(|flag| flag.reason).collect();
    reasons.extend(set_aside);
    Disposition {
        revision_id,
        outcome: decided,
        reasons,
        rule_ids,
        decided_by: GATE.to_owned(),
    }
}

/// How severe `outcome` is: its place from `accepted`, the least, to
/// `excluded`, the most.
fn severity(outcome: Outcome) -> usize {
    outcome::ALL
        .iter()
        .position(|named| *named == outcome)
        .unwrap_or_default()
}
