//! What the gate reports of a collection: every revision counted once, by
//! outcome and by rule, and each one held back listed with why.

use super::outcome;
use maestro_kernel::document::{Disposition, Outcome, Revision};
use serde::{Serialize, Serializer};
use serde_json::Value;
use std::collections::BTreeMap;

/// What a run of the gate over a collection found: each revision the caller
/// reads counted once, decided by this run or kept as decided before. It
/// serializes as JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The collection.
    pub collection: String,
    /// Its revisions, failed ones included.
    pub revisions: u64,
    /// The revisions this run decided.
    pub decided: u64,
    /// The revisions whose disposition was recorded before, which they keep.
    pub kept: u64,
    /// The revisions by outcome, whoever decided them.
    pub outcomes: Outcomes,
    /// Each rule ID with the number of revisions whose disposition names it.
    pub rules: BTreeMap<String, u64>,
    /// Each ledger rule's ID with the number of revisions decided before
    /// that it matches first and would give another outcome: a recorded
    /// disposition is kept, so the rule changed none of them.
    pub ignored_rules: BTreeMap<String, u64>,
    /// Each revision held back, in record order.
    pub held: Vec<Held>,
}

impl Report {
    /// The report of a run over `collection` that has counted nothing yet.
    pub(super) fn new(collection: &str) -> Self {
        Self {
            collection: collection.to_owned(),
            revisions: 0,
            decided: 0,
            kept: 0,
            outcomes: Outcomes::default(),
            rules: BTreeMap::new(),
            ignored_rules: BTreeMap::new(),
            held: Vec::new(),
        }
    }

    /// Counts a revision decided before that the ledger rule `rule` would
    /// have given another outcome.
    pub(super) fn ignore(&mut self, rule: String) {
        *self.ignored_rules.entry(rule).or_default() += 1;
    }

    /// Counts `revision`, whose document has `source_ref`, with its
    /// `disposition`, which this run `decided` or kept.
    pub(super) fn count(
        &mut self,
        revision: &Revision,
        source_ref: &str,
        disposition: Disposition,
        decided: bool,
    ) {
        self.revisions += 1;
        if decided {
            self.decided += 1;
        } else {
            self.kept += 1;
        }
        *self.outcomes.of(disposition.outcome) += 1;
        for rule in &disposition.rule_ids {
            *self.rules.entry(rule.clone()).or_default() += 1;
        }
        if outcome::holds(disposition.outcome) {
            let text = |field: &str| revision.metadata.get(field).and_then(Value::as_str);
            self.held.push(Held {
                revision: disposition.revision_id,
                document: revision.document_id.clone(),
                source_ref: source_ref.to_owned(),
                source_kind: text("source_kind").map(str::to_owned),
                set: text("set").map(str::to_owned),
                outcome: disposition.outcome,
                rule_ids: disposition.rule_ids,
                reasons: disposition.reasons,
                decided_by: disposition.decided_by,
            });
        }
    }
}

/// The revisions of a collection by outcome.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Outcomes {
    /// `accepted`.
    pub accepted: u64,
    /// `accepted_with_warnings`.
    pub accepted_with_warnings: u64,
    /// `needs_reextraction`.
    pub needs_reextraction: u64,
    /// `quarantined`.
    pub quarantined: u64,
    /// `excluded`.
    pub excluded: u64,
}

impl Outcomes {
    /// The count of `outcome`.
    fn of(&mut self, outcome: Outcome) -> &mut u64 {
        match outcome {
            Outcome::Accepted => &mut self.accepted,
            Outcome::AcceptedWithWarnings => &mut self.accepted_with_warnings,
            Outcome::NeedsReextraction => &mut self.needs_reextraction,
            Outcome::Quarantined => &mut self.quarantined,
            Outcome::Excluded => &mut self.excluded,
        }
    }
}

/// A revision held back from indexing, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Held {
    /// The revision's ID.
    pub revision: String,
    /// Its document's ID.
    pub document: String,
    /// Its document's `source_ref`.
    pub source_ref: String,
    /// The kind of source it comes from, as its manifest line gave it.
    pub source_kind: Option<String>,
    /// The document set it belongs to, as its manifest line gave it.
    pub set: Option<String>,
    /// Its outcome, which the JSON gives by name.
    #[serde(serialize_with = "named")]
    pub outcome: Outcome,
    /// The rules that gave it.
    pub rule_ids: Vec<String>,
    /// Why, for people.
    pub reasons: Vec<String>,
    /// Who decided it: the gate, the import or a person.
    pub decided_by: String,
}

/// `outcome` by its name.
#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's serialize_with hands the field over by reference"
)]
fn named<S: Serializer>(outcome: &Outcome, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(outcome::name(*outcome))
}
