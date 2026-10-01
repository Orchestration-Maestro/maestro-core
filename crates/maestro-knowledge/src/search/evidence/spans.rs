//! Candidate source-span unions.

use super::super::fusion::Route;
use maestro_kernel::evidence::Span;
use std::collections::BTreeSet;

/// A retrieved chunk's source span and provenance.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SeedSpan {
    /// The ID used to retrieve the chunk.
    pub(crate) chunk_id: String,
    /// Revision whose original bytes contain the span.
    pub(crate) revision_id: String,
    /// Canonical section associated with the chunk, if any.
    pub(crate) section_id: Option<String>,
    /// Half-open UTF-8 byte interval in the original source.
    pub(crate) span: Span,
    /// Original candidate position before any reordering.
    pub(crate) input_position: usize,
    /// Reranker score, absent for the unscored tail.
    pub(crate) score: Option<f64>,
    /// Routes that returned this chunk.
    pub(crate) routes: BTreeSet<Route>,
}

/// A transitive, adjacent-or-overlapping union of seeds in one revision.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SpanUnion {
    /// Revision containing every seed in the union.
    pub(crate) revision_id: String,
    /// Combined half-open source interval.
    pub(crate) span: Span,
    /// Every candidate seed covered by the interval.
    pub(crate) seeds: Vec<SeedSpan>,
}

/// Unions touching intervals while preserving their candidate references.
pub(crate) fn union_seed_spans(mut seeds: Vec<SeedSpan>) -> Result<Vec<SpanUnion>, String> {
    if seeds.iter().any(|seed| seed.span.start >= seed.span.end) {
        return Err("seed spans must have positive length".to_owned());
    }
    seeds.sort_by(|left, right| {
        left.revision_id
            .cmp(&right.revision_id)
            .then_with(|| left.span.start.cmp(&right.span.start))
            .then_with(|| left.span.end.cmp(&right.span.end))
            .then_with(|| left.input_position.cmp(&right.input_position))
    });

    let mut unions: Vec<SpanUnion> = Vec::new();
    for seed in seeds {
        if let Some(union) = unions.last_mut().filter(|union| {
            union.revision_id == seed.revision_id && seed.span.start <= union.span.end
        }) {
            union.span.end = union.span.end.max(seed.span.end);
            union.seeds.push(seed);
        } else {
            unions.push(SpanUnion {
                revision_id: seed.revision_id.clone(),
                span: seed.span,
                seeds: vec![seed],
            });
        }
    }
    Ok(unions)
}
