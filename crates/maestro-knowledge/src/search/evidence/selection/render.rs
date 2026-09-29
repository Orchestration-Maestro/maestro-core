//! Renders trial spans as normalized, validated passages.

use super::super::selection_candidate::SelectionCandidate;
use super::super::{
    delivery_graph::{DeliveryChoice, PrimaryContribution},
    sections::contains,
    signals::{PassageOrder, order_passages},
};
use super::render_cluster::render_cluster;
use maestro_kernel::evidence::{Passage, Span};
use std::collections::{BTreeMap, BTreeSet};

/// Passages rendered from a proposed set of selected source spans.
#[derive(Default)]
pub(super) struct RenderedTrial {
    /// Reading-ordered complete passages with one-based numbers.
    pub(super) passages: Vec<Passage>,
    /// Candidate seeds whose required spans are physically represented.
    pub(super) covered_candidates: BTreeSet<usize>,
}

/// Renders and orders one budget trial from selected candidate source spans.
pub(super) fn render_trial(
    candidates: &[SelectionCandidate<'_>],
    selected_spans: &BTreeMap<usize, DeliveryChoice>,
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
    for (index, choice) in selected_spans {
        let candidate = candidates
            .get(*index)
            .ok_or_else(|| "selected candidate disappeared".to_owned())?;
        if choice_covered(candidate, &choice.ranges(), &passages) {
            covered_candidates.insert(*index);
        } else {
            covered_candidates.remove(index);
        }
    }
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
pub(super) fn group_selected_spans(
    candidates: &[SelectionCandidate<'_>],
    selected_spans: &BTreeMap<usize, DeliveryChoice>,
) -> Result<RevisionSelections, String> {
    let mut revisions = RevisionSelections::new();
    for (index, choice) in selected_spans {
        let candidate = candidates
            .get(*index)
            .ok_or_else(|| "selected candidate index is invalid".to_owned())?;
        choice.validate(candidate)?;
        if candidate.document.document_id != candidate.template.document_id
            || candidate.document.revision_id != candidate.template.revision_id
            || candidate.seeds.revision_id != candidate.template.revision_id
        {
            return Err("selected source identity is invalid".to_owned());
        }
        for span in choice.ranges() {
            revisions
                .entry((
                    candidate.template.document_id.clone(),
                    candidate.template.revision_id.clone(),
                ))
                .or_default()
                .push((*index, span));
        }
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
    for (_, span) in &selected {
        if let Some(current) = clusters
            .last_mut()
            .filter(|current: &&mut Span| span.start <= current.end)
        {
            current.end = current.end.max(span.end);
        } else {
            clusters.push(*span);
        }
    }

    let mut rendered = RenderedRevision::default();
    for span in clusters {
        let passage = render_cluster(
            candidates,
            (document_id, revision_id),
            span,
            &mut rendered.covered_candidates,
            &selected,
        )?;
        rendered.passages.push(passage);
    }
    Ok(rendered)
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

/// Maps parent-only passages to admitted primary seeds, without pretending containment.
pub(super) fn parent_supports(
    passages: &[Passage],
    candidates: &[SelectionCandidate<'_>],
    selected: &BTreeMap<usize, DeliveryChoice>,
) -> BTreeMap<u32, Vec<String>> {
    let mut supports = BTreeMap::new();
    for passage in passages {
        let mut ids = BTreeSet::new();
        for (index, choice) in selected {
            let Some(candidate) = candidates.get(*index) else {
                continue;
            };
            if candidate.template.revision_id != passage.revision_id
                || choice
                    .primary
                    .iter()
                    .any(|part| contains(passage.span, part.span))
                || !choice
                    .context
                    .iter()
                    .any(|range| contains(passage.span, *range))
            {
                continue;
            }
            ids.extend(choice.primary.iter().map(|part| part.chunk_id.clone()));
        }
        if !ids.is_empty() {
            supports.insert(passage.n, ids.into_iter().collect());
        }
    }
    supports
}

/// Requires every context range, not merely the primary seed, in the complete trial.
fn choice_covered(
    candidate: &SelectionCandidate<'_>,
    ranges: &[Span],
    passages: &[Passage],
) -> bool {
    ranges.iter().all(|range| {
        passages.iter().any(|passage| {
            passage.revision_id == candidate.template.revision_id && contains(passage.span, *range)
        })
    })
}

/// Retains seed-linked primary parts for exact per-passage trace attribution.
pub(super) fn primary_contributions(
    passages: &[Passage],
    candidates: &[SelectionCandidate<'_>],
    selected: &BTreeMap<usize, DeliveryChoice>,
) -> BTreeMap<u32, Vec<PrimaryContribution>> {
    let mut result = BTreeMap::new();
    for passage in passages {
        let mut parts = Vec::new();
        for (index, choice) in selected {
            let Some(candidate) = candidates.get(*index) else {
                continue;
            };
            if candidate.template.revision_id == passage.revision_id {
                parts.extend(
                    choice
                        .primary
                        .iter()
                        .filter(|part| contains(passage.span, part.span))
                        .cloned(),
                );
            }
        }
        if !parts.is_empty() {
            result.insert(passage.n, parts);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::evidence::delivery_graph::ChoiceKind;
    use crate::search::{
        Route,
        evidence::{
            features::diversity_features,
            sections::SectionIndex,
            spans::{SeedSpan, SpanUnion},
        },
    };
    use maestro_canonicalization::{CanonicalDocument, CanonicalizeInput, canonicalize};
    use maestro_kernel::artifact::Digest;
    use std::slice;

    fn document(markdown: &str) -> CanonicalDocument {
        canonicalize(CanonicalizeInput::new(markdown, "guide.md")).unwrap()
    }

    fn candidate<'a>(
        markdown: &'a str,
        document: &'a CanonicalDocument,
        sections: &'a SectionIndex,
        input_position: usize,
        marker: &str,
    ) -> SelectionCandidate<'a> {
        let start = markdown.find(marker).unwrap();
        let span = Span {
            start,
            end: start + marker.len(),
        };
        let section_id = document
            .sections
            .iter()
            .find(|section| section.title == "Guide")
            .map(|section| section.section_id.clone());
        let seeds = SpanUnion {
            revision_id: document.revision_id.clone(),
            span,
            seeds: vec![SeedSpan {
                chunk_id: format!("chunk-{input_position}"),
                revision_id: document.revision_id.clone(),
                section_id,
                span,
                input_position,
                score: Some(0.8),
                routes: BTreeSet::from([Route::Lexical]),
            }],
        };
        let expansion = sections.expand(&seeds).unwrap();
        let text = markdown
            .get(expansion.extent.start..expansion.extent.end)
            .unwrap();
        SelectionCandidate {
            markdown,
            document,
            sections,
            seeds,
            required_span: span,
            features: diversity_features(markdown, document, expansion.extent, None).unwrap(),
            template: Passage {
                n: 0,
                section_id: expansion.section_id.clone(),
                document_id: document.document_id.clone(),
                revision_id: document.revision_id.clone(),
                title: "Guide".to_owned(),
                section_path: expansion.section_path.clone(),
                version: None,
                source_ref: "corpus-path:guide.md".to_owned(),
                span: expansion.extent,
                digest: Digest::of(text.as_bytes()),
                text: text.to_owned(),
                windowed: false,
                alternates: Vec::new(),
            },
            expansion,
            input_position,
        }
    }

    fn assert_grouping_rejects(candidate: &SelectionCandidate<'_>, span: Span) {
        assert!(
            group_selected_spans(
                slice::from_ref(candidate),
                &BTreeMap::from([(
                    0,
                    DeliveryChoice::canonical(candidate, vec![span], ChoiceKind::Unit)
                )])
            )
            .is_err()
        );
    }

    fn assert_cluster_excludes_identity(candidates: &[SelectionCandidate<'_>]) {
        let first = &candidates[0];
        let mut covered = BTreeSet::new();
        render_cluster(
            candidates,
            (&first.template.document_id, &first.template.revision_id),
            first.expansion.extent,
            &mut covered,
            &[],
        )
        .unwrap();
        assert_eq!(covered, BTreeSet::from([0]));
    }

    fn assert_source_references_rejected(candidates: &[SelectionCandidate<'_>]) {
        let first = &candidates[0];
        assert_eq!(
            render_cluster(
                candidates,
                (&first.template.document_id, &first.template.revision_id),
                first.expansion.extent,
                &mut BTreeSet::new(),
                &[],
            )
            .err(),
            Some("one revision has inconsistent cached source references".to_owned())
        );
    }

    #[test]
    fn include_span_rejects_each_empty_input_interval() {
        let error = "selected window span is empty or reversed";
        assert_eq!(
            include_span(Span { start: 1, end: 1 }, Span { start: 1, end: 2 },).unwrap_err(),
            error
        );
        assert_eq!(
            include_span(Span { start: 1, end: 2 }, Span { start: 2, end: 2 },).unwrap_err(),
            error
        );
    }

    #[test]
    fn rejects_each_invalid_selected_span_or_source_identity() {
        let markdown = "Preface.\n\n# Guide\n\nUnicode Ω marker.\n";
        let document = document(markdown);
        let sections = SectionIndex::new(&document, markdown).unwrap();
        let valid = candidate(markdown, &document, &sections, 0, "Ω marker");
        let marker = markdown.find('Ω').unwrap();
        for invalid in [
            Span {
                start: valid.required_span.start,
                end: valid.required_span.start,
            },
            Span {
                start: marker + 1,
                end: marker + 2,
            },
            Span {
                start: 0,
                end: markdown.find("# Guide").unwrap() - 1,
            },
        ] {
            assert_grouping_rejects(&valid, invalid);
        }

        let mut wrong_document = candidate(markdown, &document, &sections, 0, "Ω marker");
        wrong_document.template.document_id = "other-document".to_owned();
        assert_grouping_rejects(&wrong_document, valid.required_span);

        let mut wrong_revision = candidate(markdown, &document, &sections, 0, "Ω marker");
        wrong_revision.template.revision_id = "other-revision".to_owned();
        assert_grouping_rejects(&wrong_revision, valid.required_span);

        let mut wrong_seed = candidate(markdown, &document, &sections, 0, "Ω marker");
        wrong_seed.seeds.revision_id = "other-revision".to_owned();
        assert_grouping_rejects(&wrong_seed, valid.required_span);
    }

    #[test]
    fn cluster_members_require_both_matching_source_ids() {
        let markdown = "# Guide\n\nFirst marker.\n\nSecond marker.\n";
        let document = document(markdown);
        let sections = SectionIndex::new(&document, markdown).unwrap();
        let first = candidate(markdown, &document, &sections, 0, "First marker.");
        let mut wrong_document = candidate(markdown, &document, &sections, 1, "Second marker.");
        wrong_document.template.document_id = "other-document".to_owned();
        assert_cluster_excludes_identity(&[first, wrong_document]);

        let first = candidate(markdown, &document, &sections, 0, "First marker.");
        let mut wrong_revision = candidate(markdown, &document, &sections, 1, "Second marker.");
        wrong_revision.template.revision_id = "other-revision".to_owned();
        assert_cluster_excludes_identity(&[first, wrong_revision]);
    }

    #[test]
    fn source_allocations_must_match_independently() {
        let markdown_a = String::from("# Guide\n\nFirst marker.\n\nSecond marker.\n");
        let markdown_b = markdown_a.clone();
        let document_a = document(&markdown_a);
        let document_b = document(&markdown_a);
        let sections_a = SectionIndex::new(&document_a, &markdown_a).unwrap();
        let sections_b = SectionIndex::new(&document_b, &markdown_a).unwrap();
        assert_eq!(document_a.document_id, document_b.document_id);
        assert_eq!(document_a.revision_id, document_b.revision_id);

        assert_source_references_rejected(&[
            candidate(&markdown_a, &document_a, &sections_a, 0, "First marker."),
            candidate(&markdown_a, &document_b, &sections_a, 1, "Second marker."),
        ]);
        assert_source_references_rejected(&[
            candidate(&markdown_a, &document_a, &sections_a, 0, "First marker."),
            candidate(&markdown_b, &document_a, &sections_a, 1, "Second marker."),
        ]);
        assert_source_references_rejected(&[
            candidate(&markdown_a, &document_a, &sections_a, 0, "First marker."),
            candidate(&markdown_a, &document_a, &sections_b, 1, "Second marker."),
        ]);
    }

    #[test]
    fn choice_coverage_requires_the_same_revision() {
        let markdown = "# Guide\n\nΩ marker.\n";
        let document = document(markdown);
        let sections = SectionIndex::new(&document, markdown).unwrap();
        let candidate = candidate(markdown, &document, &sections, 0, "Ω marker");
        let mut passage = candidate.template.clone();
        passage.revision_id = "other-revision".to_owned();

        assert!(!choice_covered(
            &candidate,
            &[candidate.required_span],
            &[passage]
        ));
    }

    #[test]
    fn cluster_does_not_admit_an_unselected_member_by_its_range() {
        let markdown = "# Guide\n\nFirst marker.\n\nSecond marker.\n";
        let document = document(markdown);
        let sections = SectionIndex::new(&document, markdown).unwrap();
        let candidates = [
            candidate(markdown, &document, &sections, 0, "First marker."),
            candidate(markdown, &document, &sections, 1, "Second marker."),
        ];
        let selected = [(1, candidates[1].required_span)];
        let mut covered = BTreeSet::new();
        let passage = render_cluster(
            &candidates,
            (&document.document_id, &document.revision_id),
            candidates[1].required_span,
            &mut covered,
            &selected,
        )
        .unwrap();

        assert_eq!(passage.input_position, 1);
        assert_eq!(covered, BTreeSet::from([1]));
    }
}
