//! Digest-bound graph evaluation labels and construction/proof scoring.

mod gates;
mod label_format;
mod label_proofs;
mod label_types;
mod label_validation;
mod labels;
mod score;
#[cfg(test)]
mod tests;

pub use gates::{
    Cohort, Gate, GraphRoute, LatencySample, Operation, QuestionRetrieval, RefusalOutcome,
    RunEvidence, RunVerdict, RungDefinition, RungEvidence, RunsVerdict, judge_runs,
};
pub use label_types::{
    CheckError, CheckedItem, CheckedLabels, LabelCode, LabelError, LabelSummary, Located, Original,
    QuestionKind, Stage,
};
pub use labels::check_labels;
pub use score::{
    ClaimReview, ClaimVerdict, ConstructionError, FamilyProof, GAIN_SEED, GainError, GainScore,
    ProofScore, ScoreConstruction, Triple, proof_complete, proof_gain, score_construction,
    score_proofs,
};
