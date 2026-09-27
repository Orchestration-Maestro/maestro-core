//! Owned intermediate records shared by the assembly worker's small modules.

use super::super::{
    conflicts::ConflictEmission,
    families::CandidateFamily,
    features::DiversityFeatures,
    sections::Expansion,
    signals::OmissionStatus,
    spans::{SeedSpan, SpanUnion},
};
use maestro_kernel::{chunk_set::Chunk, evidence::Passage};
use std::collections::BTreeSet;

/// Candidate source data after scoped lookup and its stable rank are joined.
pub(super) struct LoadedCandidate {
    /// The named chunk returned by the pinned-generation reader.
    pub(super) chunk: Chunk,
    /// Its original zero-based position in the handoff.
    pub(super) input_position: usize,
    /// The reranker score, absent for the unscored tail.
    pub(super) score: Option<f64>,
    /// The routes whose ranks contributed this candidate.
    pub(super) routes: BTreeSet<super::super::super::fusion::Route>,
}

/// One source-span union and its precomputed selection and family features.
pub(super) struct CandidateData {
    /// Every source seed represented by this candidate.
    pub(super) seeds: SpanUnion,
    /// The enclosing canonical section or content extent.
    pub(super) expansion: Expansion,
    /// Manifest-authorized identity for version and conflict matching.
    pub(super) family: CandidateFamily,
    /// Passage fields copied from authorized source records.
    pub(super) template: Passage,
    /// Full-section features for MMR ordering.
    pub(super) features: DiversityFeatures,
    /// Best original candidate position represented by this union.
    pub(super) input_position: usize,
}

/// Final bundle inputs after candidate selection and conflict numbering.
pub(super) struct BundleParts<'a> {
    /// Final source passages in reading order.
    pub(super) passages: Vec<Passage>,
    /// Every loaded primary chunk seed, before candidate suppression.
    pub(super) seeds: Vec<SeedSpan>,
    /// Authorized proposed source text, used only to classify identifier gaps.
    pub(super) candidate_texts: Vec<String>,
    /// Passage-numbered conflicts and same-passage disagreements.
    pub(super) emission: ConflictEmission<'a>,
    /// Budgeted omissions from the final selection.
    pub(super) omissions: OmissionStatus,
    /// Whether an unfiltered family could not be ordered numerically.
    pub(super) latest_undetermined: bool,
}
