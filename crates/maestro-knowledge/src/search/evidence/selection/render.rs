//! Renders trial spans as normalized, validated passages.

use super::super::{
    sections::{contains, valid_span},
    signals::{PassageOrder, order_passages},
};
use super::types::SelectionCandidate;
use maestro_kernel::{
    artifact::Digest,
    evidence::{Passage, Span},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    ptr,
};

/// Passages rendered from a proposed set of selected source spans.
pub(super) struct RenderedTrial {
    /// Reading-ordered complete passages with one-based numbers.
    pub(super) passages: Vec<Passage>,
    /// Candidate seeds whose required spans are physically represented.
    pub(super) covered_candidates: BTreeSet<usize>,
}

/// Renders and orders one budget trial from selected candidate source spans.
pub(super) fn render_trial(
    candidates: &[SelectionCandidate<'_>],
    selected_spans: &BTreeMap<usize, Span>,
) -> Result<RenderedTrial, String> {
    let revisions = group_selected_spans(candidates, selected_spans)?;
    let mut ordered = Vec::new();
    let mut covered_candidates = BTreeSet::new();
    for ((document_id, revision_id), selected) in revisions {
        let mut rendered = render_revision(candidates, &document_id, &revision_id, selected)?;
        ordered.append(&mut rendered.passages);
        covered_candidates.append(&mut rendered.covered_candidates);
    }
    let passages = order_passages(ordered)?;
    Ok(RenderedTrial {
        passages,
        covered_candidates,
    })
}

/// Reading-ordered output and covered candidates for one source revision.
#[derive(Default)]
struct RenderedRevision {
    /// Passage drafts before final cross-revision numbering.
    passages: Vec<PassageOrder>,
    /// Candidate indexes represented in these rendered passages.
    covered_candidates: BTreeSet<usize>,
}

/// Selected spans grouped by document and revision identity.
type RevisionSelections = BTreeMap<(String, String), Vec<(usize, Span)>>;

/// Validates and groups selected windows by their authorized source revision.
fn group_selected_spans(
    candidates: &[SelectionCandidate<'_>],
    selected_spans: &BTreeMap<usize, Span>,
) -> Result<RevisionSelections, String> {
    let mut revisions = RevisionSelections::new();
    for (index, span) in selected_spans {
        let candidate = candidates
            .get(*index)
            .ok_or_else(|| "selected candidate index is invalid".to_owned())?;
        if span.start >= span.end
            || !valid_span(*span, candidate.markdown)
            || !contains(candidate.expansion.extent, *span)
            || candidate.document.document_id != candidate.template.document_id
            || candidate.document.revision_id != candidate.template.revision_id
            || candidate.seeds.revision_id != candidate.template.revision_id
        {
            return Err("selected source span or identity is invalid".to_owned());
        }
        revisions
            .entry((
                candidate.template.document_id.clone(),
                candidate.template.revision_id.clone(),
            ))
            .or_default()
            .push((*index, *span));
    }
    Ok(revisions)
}

/// Sorts and merges selected windows within one source revision.
fn render_revision(
    candidates: &[SelectionCandidate<'_>],
    document_id: &str,
    revision_id: &str,
    mut selected: Vec<(usize, Span)>,
) -> Result<RenderedRevision, String> {
    selected.sort_by_key(|(index, span)| {
        (
            span.start,
            span.end,
            candidates
                .get(*index)
                .map_or(usize::MAX, |candidate| candidate.input_position),
            *index,
        )
    });
    let mut clusters = Vec::new();
    for (_, span) in selected {
        if let Some(current) = clusters
            .last_mut()
            .filter(|current: &&mut Span| span.start <= current.end)
        {
            current.end = current.end.max(span.end);
        } else {
            clusters.push(span);
        }
    }

    let mut rendered = RenderedRevision::default();
    for span in clusters {
        let passage = render_cluster(
            candidates,
            document_id,
            revision_id,
            span,
            &mut rendered.covered_candidates,
        )?;
        rendered.passages.push(passage);
    }
    Ok(rendered)
}

/// Builds one passage from all candidate seeds covered by a merged window.
fn render_cluster(
    candidates: &[SelectionCandidate<'_>],
    document_id: &str,
    revision_id: &str,
    span: Span,
    covered_candidates: &mut BTreeSet<usize>,
) -> Result<PassageOrder, String> {
    let members: BTreeSet<_> = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            candidate.template.document_id == document_id
                && candidate.template.revision_id == revision_id
                && contains(span, candidate.required_span)
        })
        .map(|(index, _)| index)
        .collect();
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
        covered_candidates.insert(*index);
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

/// Builds a valid half-open source interval containing all required spans.
pub(super) fn include_span(current: Span, addition: Span) -> Result<Span, String> {
    if current.start >= current.end || addition.start >= addition.end {
        return Err("selected window span is empty or reversed".to_owned());
    }
    Ok(Span {
        start: current.start.min(addition.start),
        end: current.end.max(addition.end),
    })
}
