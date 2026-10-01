//! Shared candidate-family identity for conflicts and documentary versions.

use maestro_canonicalization::CanonicalDocument;
use std::collections::{BTreeMap, BTreeSet};

/// Exact product context required for two revisions to correspond.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ConflictContext {
    /// Product metadata, if present.
    pub(crate) product: Option<String>,
    /// Component metadata, if present.
    pub(crate) component: Option<String>,
    /// Platform metadata, if present.
    pub(crate) platform: Option<String>,
    /// Language metadata, if present.
    pub(crate) lang: Option<String>,
}

/// A candidate's authorized correspondence identity and section location.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CandidateFamily {
    /// Canonical owning document ID.
    pub(crate) document_id: String,
    /// Manifest-allowed near-duplicate groups for this request.
    pub(crate) near_group_ids: BTreeSet<String>,
    /// Canonical heading path.
    pub(crate) section_path: Vec<String>,
    /// One-based occurrence of this path in canonical section order.
    pub(crate) occurrence: usize,
    /// Exact product context.
    pub(crate) context: ConflictContext,
}

/// Canonical section location and its occurrence among repeated heading paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SectionOccurrence {
    /// Canonical heading path.
    pub(crate) section_path: Vec<String>,
    /// One-based occurrence of this path in document order.
    pub(crate) occurrence: usize,
}

/// Returns canonical section occurrences without rescanning sections per fact.
pub(crate) fn section_occurrences(
    document: &CanonicalDocument,
) -> Result<BTreeMap<String, SectionOccurrence>, String> {
    let mut counts = BTreeMap::<Vec<String>, usize>::new();
    let mut occurrences = BTreeMap::new();
    for section in &document.sections {
        if section.section_id.trim().is_empty() {
            return Err("canonical section ID is blank".to_owned());
        }
        let count = counts.entry(section.heading_path.clone()).or_default();
        *count = count
            .checked_add(1)
            .ok_or_else(|| "canonical section occurrence overflowed".to_owned())?;
        let occurrence = SectionOccurrence {
            section_path: section.heading_path.clone(),
            occurrence: *count,
        };
        if occurrences
            .insert(section.section_id.clone(), occurrence)
            .is_some()
        {
            return Err("canonical section IDs are not unique".to_owned());
        }
    }
    Ok(occurrences)
}

/// Tests shared document or manifest-group identity at the same section location.
pub(crate) fn corresponds(left: &CandidateFamily, right: &CandidateFamily) -> bool {
    left.section_path == right.section_path
        && left.occurrence == right.occurrence
        && left.context == right.context
        && (left.document_id == right.document_id
            || left
                .near_group_ids
                .intersection(&right.near_group_ids)
                .next()
                .is_some())
}
