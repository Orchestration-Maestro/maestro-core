use super::super::{
    EvidenceCounter, EvidenceError, ExpansionMode, ParentChainOrder,
    budget::{count_passages, counter_info},
    delivery_graph::{ChoiceKind, DeliveryChoice, DeliveryGraph, PrimaryContribution},
    selection::{SelectionBudget, SelectionCandidate, select},
};
use super::{
    selection::{CandidateSource, candidate, prepared},
    support::control,
};
use maestro_kernel::{artifact::Digest, evidence::Span};
use std::collections::BTreeSet;

struct Graph(Vec<Vec<Vec<Span>>>);
impl DeliveryGraph for Graph {
    fn choices(
        &self,
        candidate: &SelectionCandidate<'_>,
    ) -> Result<Vec<DeliveryChoice>, EvidenceError> {
        Ok(self.0[candidate.input_position]
            .iter()
            .map(|ranges| DeliveryChoice {
                primary: if ranges.len() > 1 {
                    ranges
                        .iter()
                        .map(|span| PrimaryContribution {
                            chunk_id: candidate.seeds.seeds[0].chunk_id.clone(),
                            span: *span,
                        })
                        .collect()
                } else {
                    candidate
                        .seeds
                        .seeds
                        .iter()
                        .map(|seed| PrimaryContribution {
                            chunk_id: seed.chunk_id.clone(),
                            span: seed.span,
                        })
                        .collect()
                },
                context: ranges.clone(),
                kind: ChoiceKind::Unit,
            })
            .collect())
    }
}

fn span(start: usize, end: usize) -> Span {
    Span { start, end }
}

fn seeded(source: CandidateSource<'_>, position: usize, primary: Span) -> SelectionCandidate<'_> {
    let mut result = candidate(source, "Guide", "Guide", position, None);
    result.required_span = primary;
    result.seeds.span = primary;
    result.seeds.seeds[0].span = primary;
    result
}

fn cost(candidate: &SelectionCandidate<'_>, ranges: &[Span]) -> u32 {
    let passages: Vec<_> = ranges
        .iter()
        .enumerate()
        .map(|(index, range)| {
            let mut passage = candidate.template.clone();
            passage.n = u32::try_from(index + 1).unwrap();
            passage.span = *range;
            passage.text = candidate.markdown[range.start..range.end].to_owned();
            passage.digest = Digest::of(passage.text.as_bytes());
            passage.windowed = true;
            passage
        })
        .collect();
    let counter = EvidenceCounter::Utf8Bytes;
    count_passages(
        &passages,
        &counter,
        &counter_info(&counter).unwrap(),
        u32::MAX,
    )
    .unwrap()
}

#[test]
fn unequal_chains_admit_the_same_middle_global_tier() {
    let markdown = format!("# Guide\n\n{}", "abcdefghij".repeat(20));
    let (document, sections) = prepared(&markdown, "global.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [
        seeded(source, 0, span(0, 10)),
        seeded(source, 1, span(100, 110)),
    ];
    let graph = Graph(vec![
        vec![vec![span(0, 10)], vec![span(0, 30)], vec![span(0, 90)]],
        vec![vec![span(100, 110)], vec![span(100, 140)]],
    ]);
    let expected = [span(0, 30), span(100, 140)];
    let counter = EvidenceCounter::Utf8Bytes;
    let info = counter_info(&counter).unwrap();
    for order in [
        ParentChainOrder::MinimumCompleteFirst,
        ParentChainOrder::LargestFittingParent,
    ] {
        let result = select(
            &candidates,
            &[BTreeSet::from([0, 1])],
            &SelectionBudget {
                expansion: ExpansionMode::ParentChain,
                graph: &graph,
                parent_chain_order: order,
                max_passages: 2,
                max_tokens: cost(&candidates[0], &expected),
                counter: &counter,
                counter_info: &info,
                control: &control(),
            },
        )
        .unwrap();
        assert_eq!(
            result
                .passages
                .iter()
                .map(|passage| passage.span)
                .collect::<Vec<_>>(),
            expected
        );
    }
}

#[test]
fn largest_first_retries_parent_after_a_later_seed_merges_passages() {
    let markdown = format!("# Guide\n\n{}", "abcdefghij".repeat(20));
    let (document, sections) = prepared(&markdown, "growth.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [
        seeded(source, 0, span(0, 10)),
        seeded(source, 1, span(60, 70)),
        seeded(source, 2, span(10, 60)),
    ];
    let graph = Graph(vec![
        vec![vec![span(0, 10)]],
        vec![vec![span(60, 70)], vec![span(50, 90)]],
        vec![vec![span(10, 60)]],
    ]);
    let budget = cost(&candidates[0], &[span(0, 10), span(50, 90)]) - 1;
    assert!(cost(&candidates[0], &[span(0, 10), span(60, 70)]) <= budget);
    assert!(cost(&candidates[0], &[span(0, 90)]) <= budget);
    let counter = EvidenceCounter::Utf8Bytes;
    let result = select(
        &candidates,
        &[],
        &SelectionBudget {
            expansion: ExpansionMode::ParentChain,
            graph: &graph,
            parent_chain_order: ParentChainOrder::LargestFittingParent,
            max_passages: 5,
            max_tokens: budget,
            counter: &counter,
            counter_info: &counter_info(&counter).unwrap(),
            control: &control(),
        },
    )
    .unwrap();
    assert_eq!(result.selected_candidates, BTreeSet::from([0, 1, 2]));
    assert_eq!(
        result
            .passages
            .iter()
            .map(|passage| passage.span)
            .collect::<Vec<_>>(),
        [span(0, 90)]
    );
}

#[test]
fn persisted_adapter_can_cover_one_seed_with_two_disjoint_primary_parts() {
    let markdown = format!("# Guide\n\n{}", "abcdefghij".repeat(20));
    let (document, sections) = prepared(&markdown, "parts.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [seeded(source, 0, span(0, 70))];
    let graph = Graph(vec![vec![vec![span(0, 10), span(60, 70)]]]);
    let counter = EvidenceCounter::Utf8Bytes;
    let result = select(
        &candidates,
        &[],
        &SelectionBudget {
            expansion: ExpansionMode::ParentChain,
            graph: &graph,
            parent_chain_order: ParentChainOrder::default(),
            max_passages: 5,
            max_tokens: 2000,
            counter: &counter,
            counter_info: &counter_info(&counter).unwrap(),
            control: &control(),
        },
    )
    .unwrap();
    assert_eq!(result.passages.len(), 2);
    assert_eq!(result.selected_candidates, BTreeSet::from([0]));
    assert!(result.parent_supports.is_empty());
    for passage in &result.passages {
        let primary = &result.primary_contributions[&passage.n];
        assert_eq!(primary.len(), 1);
        assert_eq!(primary[0].span, passage.span);
        let trace = super::super::signals::trace_for_passage(&super::super::signals::TraceInput {
            number: passage.n,
            revision_id: &document.revision_id,
            span: passage.span,
            seeds: &candidates[0].seeds.seeds,
            parent_context_of: &[],
            primary,
            document: &document,
            markdown: &markdown,
        })
        .unwrap();
        assert_eq!(trace.chunk_ids, ["chunk-0"]);
        assert!(trace.parent_context_of.is_empty());
    }
}
