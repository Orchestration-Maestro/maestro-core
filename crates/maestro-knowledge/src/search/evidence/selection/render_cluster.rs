//! Renders one exact source cluster and attributes supporting candidates.

use super::super::{
    sections::contains, selection_candidate::SelectionCandidate, signals::PassageOrder,
};
use maestro_kernel::{artifact::Digest, evidence::Span};
use std::{collections::BTreeSet, ptr};

/// Builds one passage from all candidate seeds covered by a merged window.
/// Selected ranges must be grouped to `identity` by `group_selected_spans`.
pub(super) fn render_cluster(
    candidates: &[SelectionCandidate<'_>],
    identity: (&str, &str),
    span: Span,
    covered_candidates: &mut BTreeSet<usize>,
    selected: &[(usize, Span)],
) -> Result<PassageOrder, String> {
    let (document_id, revision_id) = identity;
    let mut members: BTreeSet<_> = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            candidate.template.document_id == document_id
                && candidate.template.revision_id == revision_id
                && contains(span, candidate.required_span)
        })
        .map(|(index, _)| index)
        .collect();
    members.extend(selected.iter().filter_map(|(index, range)| {
        candidates
            .get(*index)
            .and_then(|_| contains(span, *range).then_some(*index))
    }));
    if members.is_empty() {
        return Err("selected passage contains no retained candidate seed".to_owned());
    }
    let representative = members
        .iter()
        .filter_map(|index| {
            candidates
                .get(*index)
                .map(|candidate| (candidate.input_position, *index))
        })
        .min()
        .map(|(_, index)| index)
        .ok_or_else(|| "selected passage has no source candidate".to_owned())?;
    let source = candidates
        .get(representative)
        .ok_or_else(|| "selected source candidate disappeared".to_owned())?;
    let mut seeds = Vec::new();
    let mut alternates = Vec::new();
    let mut input_position = usize::MAX;
    for index in &members {
        let candidate = candidates
            .get(*index)
            .ok_or_else(|| "covered candidate index is invalid".to_owned())?;
        validate_source_references(source, candidate)?;
        seeds.extend(candidate.seeds.seeds.iter().cloned());
        alternates.extend(candidate.template.alternates.iter().cloned());
        input_position = input_position.min(candidate.input_position);
        if contains(span, candidate.required_span) {
            covered_candidates.insert(*index);
        }
    }
    let union = super::super::spans::SpanUnion {
        revision_id: revision_id.to_owned(),
        span,
        seeds,
    };
    let expansion = source
        .sections
        .expand(&union)
        .map_err(|_| "selected passage has no valid enclosing section".to_owned())?;
    let text = source
        .markdown
        .get(span.start..span.end)
        .ok_or_else(|| "selected passage is not a UTF-8 source span".to_owned())?;
    alternates.sort_by(|left, right| {
        left.version
            .cmp(&right.version)
            .then_with(|| left.section_id.cmp(&right.section_id))
    });
    alternates.dedup();
    let windowed = expansion.is_windowed(span);
    let mut passage = source.template.clone();
    passage.n = 0;
    passage.section_id = expansion.section_id;
    passage.section_path = expansion.section_path;
    passage.span = span;
    passage.digest = Digest::of(text.as_bytes());
    text.clone_into(&mut passage.text);
    passage.windowed = windowed;
    passage.alternates = alternates.into_iter().collect();
    Ok(PassageOrder {
        passage,
        input_position,
    })
}

/// Rejects candidates that do not share the same cached source allocations.
fn validate_source_references(
    source: &SelectionCandidate<'_>,
    candidate: &SelectionCandidate<'_>,
) -> Result<(), String> {
    if !ptr::eq(source.document, candidate.document)
        || !ptr::eq(source.markdown, candidate.markdown)
        || !ptr::eq(source.sections, candidate.sections)
    {
        return Err("one revision has inconsistent cached source references".to_owned());
    }
    Ok(())
}
