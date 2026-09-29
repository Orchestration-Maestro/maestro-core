//! Replaceable delivery choices derived from validated canonical structure.

use super::{
    sections::{block_span, contains, valid_span},
    selection_candidate::SelectionCandidate,
    types::EvidenceError,
};
use maestro_canonicalization::BlockType;
use maestro_kernel::evidence::Span;

/// One complete source unit and its required context, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DeliveryChoice {
    /// Exact primary source contributions, linked to retrieved seeds.
    pub(crate) primary: Vec<PrimaryContribution>,
    /// Required parent or whole-block context, which may contain primary bytes.
    pub(crate) context: Vec<Span>,
    /// Structural level represented by this choice.
    pub(crate) kind: ChoiceKind,
}

/// An exact primary part of a retrieved seed, not its possibly gapped envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PrimaryContribution {
    /// Retrieved seed whose source membership authorizes this contribution.
    pub(crate) chunk_id: String,
    /// Exact source bytes contributed by this seed.
    pub(crate) span: Span,
}

impl DeliveryChoice {
    /// Legacy canonical seeds each contribute one contiguous primary range.
    pub(crate) fn canonical(
        candidate: &SelectionCandidate<'_>,
        context: Vec<Span>,
        kind: ChoiceKind,
    ) -> Self {
        Self {
            primary: candidate
                .seeds
                .seeds
                .iter()
                .map(|seed| PrimaryContribution {
                    chunk_id: seed.chunk_id.clone(),
                    span: seed.span,
                })
                .collect(),
            context,
            kind,
        }
    }

    /// Normalizes exact primary and context intervals without spanning a source gap.
    pub(crate) fn ranges(&self) -> Vec<Span> {
        let mut parts: Vec<_> = self
            .primary
            .iter()
            .map(|part| part.span)
            .chain(self.context.iter().copied())
            .collect();
        parts.sort_by_key(|span| (span.start, span.end));
        let mut ranges: Vec<Span> = Vec::new();
        for part in parts {
            if let Some(last) = ranges.last_mut().filter(|last| part.start <= last.end) {
                last.end = last.end.max(part.end);
            } else {
                ranges.push(part);
            }
        }
        ranges
    }

    /// Checks every raw interval and seed link before normalization can hide bad ranges.
    pub(crate) fn validate(&self, candidate: &SelectionCandidate<'_>) -> Result<(), String> {
        let valid = |span: Span| {
            span.start < span.end
                && valid_span(span, candidate.markdown)
                && contains(candidate.expansion.extent, span)
        };
        if self.primary.is_empty() || self.context.iter().any(|span| !valid(*span)) {
            return Err("delivery choice has invalid source ranges".to_owned());
        }
        for part in &self.primary {
            if !valid(part.span)
                || !candidate
                    .seeds
                    .seeds
                    .iter()
                    .any(|seed| seed.chunk_id == part.chunk_id && contains(seed.span, part.span))
            {
                return Err("primary contribution has no valid source seed".to_owned());
            }
        }
        if candidate.seeds.seeds.iter().any(|seed| {
            !self
                .primary
                .iter()
                .any(|part| part.chunk_id == seed.chunk_id)
        }) {
            return Err("delivery choice omits a primary seed".to_owned());
        }
        Ok(())
    }
}

/// Structural meaning of a complete delivery choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChoiceKind {
    /// A complete standalone block or procedure.
    Unit,
    /// Whole matched table rows together with their column header.
    UnitWithHeader,
    /// The complete enclosing lexical group.
    ParentGroup,
    /// The entire enclosing section.
    Section,
}

/// Supplies smallest-first, complete source choices without I/O or model calls.
pub(crate) trait DeliveryGraph {
    /// Derives complete choices from an already authorized candidate.
    fn choices(
        &self,
        candidate: &SelectionCandidate<'_>,
    ) -> Result<Vec<DeliveryChoice>, EvidenceError>;
}

/// Replays the existing canonical block tree; needs no persisted graph or re-index.
pub(crate) struct LegacyCanonicalGraph;

impl DeliveryGraph for LegacyCanonicalGraph {
    fn choices(
        &self,
        candidate: &SelectionCandidate<'_>,
    ) -> Result<Vec<DeliveryChoice>, EvidenceError> {
        let mandatory = candidate
            .expansion
            .window_plan(candidate.required_span)
            .map_err(invalid)?
            .mandatory;
        let mut choices = Vec::new();
        if let Some(ranges) = table_ranges(candidate, mandatory)? {
            choices.push(DeliveryChoice::canonical(
                candidate,
                ranges,
                ChoiceKind::UnitWithHeader,
            ));
            choices.push(DeliveryChoice::canonical(
                candidate,
                vec![mandatory],
                ChoiceKind::ParentGroup,
            ));
        } else {
            choices.push(DeliveryChoice::canonical(
                candidate,
                vec![mandatory],
                ChoiceKind::Unit,
            ));
        }
        if mandatory != candidate.expansion.extent {
            choices.push(DeliveryChoice::canonical(
                candidate,
                vec![candidate.expansion.extent],
                ChoiceKind::Section,
            ));
        }
        choices.dedup_by(|right, left| right.ranges() == left.ranges());
        Ok(choices)
    }
}

/// Splits only standalone tables: nested tables retain their procedure prerequisites.
fn table_ranges(
    candidate: &SelectionCandidate<'_>,
    mandatory: Span,
) -> Result<Option<Vec<Span>>, EvidenceError> {
    for table in candidate
        .document
        .blocks
        .iter()
        .filter(|block| block.block_type == BlockType::Table)
    {
        if block_span(table, candidate.markdown).map_err(invalid)? != Some(mandatory) {
            continue;
        }
        let mut header = None;
        let mut rows = Vec::new();
        for block in candidate
            .document
            .blocks
            .iter()
            .filter(|block| block.parent_block_id.as_deref() == Some(table.block_id.as_str()))
        {
            let Some(span) = block_span(block, candidate.markdown).map_err(invalid)? else {
                continue;
            };
            match block.block_type {
                BlockType::TableHead => header = Some(span),
                BlockType::TableRow
                    if span.start < candidate.required_span.end
                        && candidate.required_span.start < span.end =>
                {
                    rows.push(span);
                }
                _ => {}
            }
        }
        let Some(header) = header else {
            return Ok(None);
        };
        rows.push(header);
        rows.sort_by_key(|span| (span.start, span.end));
        let mut ranges: Vec<Span> = Vec::new();
        for span in rows {
            if let Some(previous) = ranges
                .last_mut()
                .filter(|previous| span.start <= previous.end)
            {
                previous.end = previous.end.max(span.end);
            } else {
                ranges.push(span);
            }
        }
        // A seed crossing the delimiter or another unrepresented gap needs the whole table.
        if !ranges
            .iter()
            .any(|span| contains(*span, candidate.required_span))
        {
            return Ok(None);
        }
        return Ok(Some(ranges));
    }
    Ok(None)
}

/// Keeps canonical diagnostics behind the stable integrity boundary.
fn invalid(_: String) -> EvidenceError {
    EvidenceError::Integrity("candidate has no complete parent-chain choice".to_owned())
}
