//! The five outcomes of the gate by the names docs/architecture/01 §4 gives
//! them, which the ledger reads and the report writes, and which of them
//! hold a revision back.

use maestro_kernel::document::Outcome;

/// Every outcome, from the one that lets a revision be indexed most freely
/// to the one that holds it back for good.
pub(super) const ALL: [Outcome; 5] = [
    Outcome::Accepted,
    Outcome::AcceptedWithWarnings,
    Outcome::NeedsReextraction,
    Outcome::Quarantined,
    Outcome::Excluded,
];

/// The name of `outcome`, as the `quality_dispositions` table spells it.
pub(super) fn name(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Accepted => "accepted",
        Outcome::AcceptedWithWarnings => "accepted_with_warnings",
        Outcome::NeedsReextraction => "needs_reextraction",
        Outcome::Quarantined => "quarantined",
        Outcome::Excluded => "excluded",
    }
}

/// Whether `outcome` holds its revision back from indexing: every outcome
/// but `accepted` and `accepted_with_warnings`.
pub(super) fn holds(outcome: Outcome) -> bool {
    !matches!(outcome, Outcome::Accepted | Outcome::AcceptedWithWarnings)
}
