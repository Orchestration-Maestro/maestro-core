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
use maestro_canonicalization::{Error, TokenCounter};
use maestro_kernel::{artifact::Digest, evidence::Span};
use std::{
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

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

struct FixedGraph(DeliveryChoice);

impl DeliveryGraph for FixedGraph {
    fn choices(
        &self,
        _candidate: &SelectionCandidate<'_>,
    ) -> Result<Vec<DeliveryChoice>, EvidenceError> {
        Ok(vec![self.0.clone()])
    }
}

struct HeaderGraph;

impl DeliveryGraph for HeaderGraph {
    fn choices(
        &self,
        candidate: &SelectionCandidate<'_>,
    ) -> Result<Vec<DeliveryChoice>, EvidenceError> {
        Ok(vec![DeliveryChoice {
            primary: candidate
                .seeds
                .seeds
                .iter()
                .map(|seed| PrimaryContribution {
                    chunk_id: seed.chunk_id.clone(),
                    span: seed.span,
                })
                .collect(),
            context: vec![candidate.required_span],
            kind: ChoiceKind::UnitWithHeader,
        }])
    }
}

struct CountingCounter(AtomicUsize);

impl TokenCounter for CountingCounter {
    fn contract_id(&self) -> &'static str {
        "test/counting"
    }

    fn verify(&self) -> Result<(), Error> {
        Ok(())
    }

    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Ok(vec![1; input.len()])
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
fn invalid_context_is_rejected_even_when_primary_is_valid() {
    let markdown = format!("# Guide\n\n{}", "abcdefghij".repeat(20));
    let (document, sections) = prepared(&markdown, "invalid-context.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidate = seeded(source, 0, span(0, 10));
    let graph = FixedGraph(DeliveryChoice::canonical(
        &candidate,
        vec![span(0, 0)],
        ChoiceKind::Unit,
    ));
    let counter = EvidenceCounter::Utf8Bytes;
    let result = select(
        &[candidate],
        &[],
        &SelectionBudget {
            expansion: ExpansionMode::ParentChain,
            graph: &graph,
            parent_chain_order: ParentChainOrder::MinimumCompleteFirst,
            max_passages: 2,
            max_tokens: 1000,
            counter: &counter,
            counter_info: &counter_info(&counter).unwrap(),
            control: &control(),
        },
    );

    assert!(matches!(
        result,
        Err(EvidenceError::Integrity(reason)) if reason == "candidate trial could not be rendered"
    ));
}

#[test]
fn each_primary_part_must_belong_to_its_own_seed() {
    let markdown = format!("# Guide\n\n{}", "abcdefghij".repeat(20));
    let (document, sections) = prepared(&markdown, "seed-links.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let mut candidate = seeded(source, 0, span(0, 30));
    candidate.seeds.span = span(0, 30);
    candidate.required_span = span(0, 30);
    candidate.seeds.seeds[0].chunk_id = "chunk-a".to_owned();
    candidate.seeds.seeds[0].span = span(0, 10);
    let mut other_seed = candidate.seeds.seeds[0].clone();
    other_seed.chunk_id = "chunk-b".to_owned();
    other_seed.span = span(20, 30);
    candidate.seeds.seeds.push(other_seed);
    let graph = FixedGraph(DeliveryChoice {
        primary: vec![
            PrimaryContribution {
                chunk_id: "chunk-a".to_owned(),
                span: span(20, 30),
            },
            PrimaryContribution {
                chunk_id: "chunk-b".to_owned(),
                span: span(20, 30),
            },
        ],
        context: vec![span(0, 30)],
        kind: ChoiceKind::Unit,
    });
    let counter = EvidenceCounter::Utf8Bytes;
    let result = select(
        &[candidate],
        &[],
        &SelectionBudget {
            expansion: ExpansionMode::ParentChain,
            graph: &graph,
            parent_chain_order: ParentChainOrder::MinimumCompleteFirst,
            max_passages: 2,
            max_tokens: 1000,
            counter: &counter,
            counter_info: &counter_info(&counter).unwrap(),
            control: &control(),
        },
    );

    assert!(matches!(
        result,
        Err(EvidenceError::Integrity(reason)) if reason == "candidate trial could not be rendered"
    ));
}

#[test]
fn omitted_conflict_counts_every_member_requiring_a_table_header() {
    let markdown = format!("# Guide\n\n{}", "abcdefghij".repeat(20));
    let (document, sections) = prepared(&markdown, "headers.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [
        seeded(source, 0, span(0, 10)),
        seeded(source, 1, span(20, 30)),
    ];
    let graph = HeaderGraph;
    let counter = EvidenceCounter::Utf8Bytes;
    let result = select(
        &candidates,
        &[BTreeSet::from([0, 1])],
        &SelectionBudget {
            expansion: ExpansionMode::ParentChain,
            graph: &graph,
            parent_chain_order: ParentChainOrder::MinimumCompleteFirst,
            max_passages: 2,
            max_tokens: 1,
            counter: &counter,
            counter_info: &counter_info(&counter).unwrap(),
            control: &control(),
        },
    )
    .unwrap();

    assert_eq!(result.omissions.table_prefix_omissions, 2);
}

#[test]
fn growth_never_recounts_the_admitted_tier() {
    let markdown = format!("# Guide\n\n{}", "abcdefghij".repeat(20));
    let (document, sections) = prepared(&markdown, "counted-growth.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [seeded(source, 0, span(0, 10))];
    let graph = Graph(vec![vec![
        vec![span(0, 10)],
        vec![span(0, 30)],
        vec![span(0, 90)],
    ]]);
    let counter = Arc::new(CountingCounter(AtomicUsize::new(0)));
    let exact = EvidenceCounter::Exact(counter.clone());
    let count = cost(&candidates[0], &[span(0, 10)]);

    select(
        &candidates,
        &[],
        &SelectionBudget {
            expansion: ExpansionMode::ParentChain,
            graph: &graph,
            parent_chain_order: ParentChainOrder::MinimumCompleteFirst,
            max_passages: 2,
            max_tokens: count,
            counter: &exact,
            counter_info: &counter_info(&exact).unwrap(),
            control: &control(),
        },
    )
    .unwrap();

    assert_eq!(counter.0.load(Ordering::Relaxed), 3);
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
