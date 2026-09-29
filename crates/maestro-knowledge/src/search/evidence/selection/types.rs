//! Candidate metadata and final selection result.

use super::super::super::assembly_settings::{ExpansionMode, ParentChainOrder};
use super::super::{
    budget::CounterInfo,
    delivery_graph::{DeliveryGraph, PrimaryContribution},
    types::{EvidenceCounter, EvidenceError},
};
use maestro_kernel::{evidence::Passage, retrieval::ReadControl};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::atomic::Ordering as AtomicOrdering,
    time::Instant,
};

/// Limits and exact counter shared by every complete-trial measurement.
pub(crate) struct SelectionBudget<'a> {
    /// Whether passage admission precedes optional context expansion.
    pub(crate) expansion: ExpansionMode,
    /// Replaceable provider of complete source choices.
    pub(crate) graph: &'a dyn DeliveryGraph,
    /// Admission order, ignored by legacy strategies.
    pub(crate) parent_chain_order: ParentChainOrder,
    /// Maximum number of returned passages.
    pub(crate) max_passages: usize,
    /// Maximum complete passage-array count.
    pub(crate) evidence_bytes: u32,
    /// Counter selected by the request.
    pub(crate) counter: &'a EvidenceCounter,
    /// Stable contract identity echoed into the bundle.
    pub(crate) counter_info: &'a CounterInfo,
    /// Shared cancellation and deadline control.
    pub(crate) control: &'a ReadControl,
}

/// Selected, ordered passages and budget omissions.
pub(crate) struct SelectionResult {
    /// Exact primary source parts represented by each passage in parent-chain mode.
    pub(crate) primary_contributions: BTreeMap<u32, Vec<PrimaryContribution>>,
    /// Explicit seed support for parent-only exact source passages.
    pub(crate) parent_supports: BTreeMap<u32, Vec<String>>,
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
