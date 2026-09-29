//! Candidate metadata and final selection result.

use super::super::super::assembly_settings::ExpansionMode;
use super::super::{
    budget::CounterInfo,
    features::DiversityFeatures,
    sections::{Expansion, SectionIndex},
    spans::SpanUnion,
    types::{EvidenceCounter, EvidenceError},
};
use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::{
    evidence::{Passage, Span},
    retrieval::ReadControl,
};
use std::{collections::BTreeSet, sync::atomic::Ordering as AtomicOrdering, time::Instant};

/// One already-validated source candidate and its full canonical expansion.
pub(crate) struct SelectionCandidate<'a> {
    /// Authoritative original Markdown bytes.
    pub(crate) markdown: &'a str,
    /// Replayed canonical structure for this source revision.
    pub(crate) document: &'a CanonicalDocument,
    /// Request-cached section index for this source revision.
    pub(crate) sections: &'a SectionIndex,
    /// Candidate chunks combined into one touching-span seed union.
    pub(crate) seeds: SpanUnion,
    /// Smallest source interval that must survive selection.
    pub(crate) required_span: Span,
    /// Full section or root-content expansion.
    pub(crate) expansion: Expansion,
    /// Precomputed full-section features used by MMR.
    pub(crate) features: DiversityFeatures,
    /// Passage metadata before selecting its final source window.
    pub(crate) template: Passage,
    /// Earliest original reranker position supporting this candidate.
    pub(crate) input_position: usize,
}

/// Limits and exact counter shared by every complete-trial measurement.
pub(crate) struct SelectionBudget<'a> {
    /// Whether passage admission precedes optional context expansion.
    pub(crate) expansion: ExpansionMode,
    /// Maximum number of returned passages.
    pub(crate) max_passages: usize,
    /// Maximum complete passage-array count.
    pub(crate) max_tokens: u32,
    /// Counter selected by the request.
    pub(crate) counter: &'a EvidenceCounter,
    /// Stable contract identity echoed into the bundle.
    pub(crate) counter_info: &'a CounterInfo,
    /// Shared cancellation and deadline control.
    pub(crate) control: &'a ReadControl,
}

/// Selected, ordered passages and budget omissions.
pub(crate) struct SelectionResult {
    /// Final passages in reading order with assigned numbers.
    pub(crate) passages: Vec<Passage>,
    /// Every candidate whose required source span is present in the result.
    pub(crate) selected_candidates: BTreeSet<usize>,
    /// Stable omission categories for known-gap generation.
    pub(crate) omissions: super::super::signals::OmissionStatus,
}

/// Stops before or after trial work when cancellation or expiry fires.
pub(super) fn check(control: &ReadControl) -> Result<(), EvidenceError> {
    if control.cancelled.load(AtomicOrdering::Relaxed) || Instant::now() >= control.deadline {
        Err(EvidenceError::TimedOut)
    } else {
        Ok(())
    }
}

/// Converts helper diagnostics to the stable, text-free integrity boundary.
pub(super) fn integrity(reason: &str) -> EvidenceError {
    EvidenceError::Integrity(reason.to_owned())
}
