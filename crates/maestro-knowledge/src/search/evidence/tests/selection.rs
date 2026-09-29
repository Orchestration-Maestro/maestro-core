#[path = "packing.rs"]
mod packing;

use super::super::{
    ExpansionMode,
    budget::{count_passages, counter_info},
    features::diversity_features,
    sections::SectionIndex,
    selection::{SelectionBudget, SelectionCandidate, select},
    spans::{SeedSpan, SpanUnion},
    types::{EvidenceCounter, EvidenceError},
};
use super::support::control;
use crate::search::Route;
use crate::search::evidence::ParentChainOrder;
use crate::search::evidence::delivery_graph::LegacyCanonicalGraph;
use maestro_canonicalization::{
    CanonicalDocument, CanonicalizeInput, Error as CanonicalError, TokenCounter, canonicalize,
};
use maestro_kernel::{
    artifact::Digest,
    evidence::{Passage, Span},
    retrieval::ReadControl,
};
use std::{
    collections::BTreeSet,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};

pub(in crate::search::evidence) fn prepared(
    markdown: &str,
    path: &str,
) -> (CanonicalDocument, SectionIndex) {
    let document = canonicalize(CanonicalizeInput::new(markdown, path)).unwrap();
    let index = SectionIndex::new(&document, markdown).unwrap();
    (document, index)
}

#[derive(Clone, Copy)]
pub(in crate::search::evidence) struct CandidateSource<'a> {
    pub(in crate::search::evidence) markdown: &'a str,
    pub(in crate::search::evidence) document: &'a CanonicalDocument,
    pub(in crate::search::evidence) sections: &'a SectionIndex,
}

pub(in crate::search::evidence) fn candidate<'a>(
    source: CandidateSource<'a>,
    section_title: &str,
    marker: &str,
    input_position: usize,
    version: Option<&str>,
) -> SelectionCandidate<'a> {
    let section_id = source
        .document
        .sections
        .iter()
        .find(|section| section.title == section_title)
        .map(|section| section.section_id.clone());
    let start = source.markdown.find(marker).unwrap();
    let span = Span {
        start,
        end: start + marker.len(),
    };
    let seeds = SpanUnion {
        revision_id: source.document.revision_id.clone(),
        span,
        seeds: vec![SeedSpan {
            chunk_id: format!("chunk-{input_position}"),
            revision_id: source.document.revision_id.clone(),
            section_id,
            span,
            input_position,
            score: Some(0.8),
            routes: BTreeSet::from([Route::Lexical]),
        }],
    };
    let expansion = source.sections.expand(&seeds).unwrap();
    let text = source
        .markdown
        .get(expansion.extent.start..expansion.extent.end)
        .unwrap();
    let version = version.map(str::to_owned);
    SelectionCandidate {
        markdown: source.markdown,
        document: source.document,
        sections: source.sections,
        seeds,
        required_span: span,
        features: diversity_features(
            source.markdown,
            source.document,
            expansion.extent,
            version.clone(),
        )
        .unwrap(),
        template: Passage {
            n: 0,
            section_id: expansion.section_id.clone(),
            document_id: source.document.document_id.clone(),
            revision_id: source.document.revision_id.clone(),
            title: "Guide".to_owned(),
            section_path: expansion.section_path.clone(),
            version,
            source_ref: format!("corpus-path:{input_position}.md"),
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

fn run_selection(
    candidates: &[SelectionCandidate<'_>],
    conflict_units: &[BTreeSet<usize>],
    max_passages: usize,
    evidence_bytes: u32,
) -> super::super::selection::SelectionResult {
    let counter = EvidenceCounter::Utf8Bytes;
    let info = counter_info(&counter).unwrap();
    let read_control = control();
    select(
        candidates,
        conflict_units,
        &SelectionBudget {
            graph: &LegacyCanonicalGraph,
            parent_chain_order: ParentChainOrder::default(),
            expansion: ExpansionMode::default(),
            max_passages,
            evidence_bytes,
            counter: &counter,
            counter_info: &info,
            control: &read_control,
        },
    )
    .unwrap()
}

struct NonMonotonicCounter;

impl TokenCounter for NonMonotonicCounter {
    fn contract_id(&self) -> &'static str {
        "selection-test/non-monotonic/1"
    }

    fn verify(&self) -> Result<(), CanonicalError> {
        Ok(())
    }

    fn token_ids(&self, input: &str) -> Result<Vec<u32>, CanonicalError> {
        let count = if input.contains("Huge after.") {
            3
        } else if input.contains("Far before.") {
            1
        } else if input.contains("Near before.") {
            3
        } else {
            2
        };
        Ok(vec![0; count])
    }
}

#[test]
fn mmr_prefers_a_distinct_third_candidate_when_two_passages_fit() {
    let markdown_a = "# Guide\n\nSame words describe the default mode.\n";
    let markdown_b = "# Guide\n\nSame words describe the default mode.\n";
    let markdown_c = "# Guide\n\nUnique procedures explain another service.\n";
    let (document_a, sections_a) = prepared(markdown_a, "a.md");
    let (document_b, sections_b) = prepared(markdown_b, "b.md");
    let (document_c, sections_c) = prepared(markdown_c, "c.md");
    let source_a = CandidateSource {
        markdown: markdown_a,
        document: &document_a,
        sections: &sections_a,
    };
    let source_b = CandidateSource {
        markdown: markdown_b,
        document: &document_b,
        sections: &sections_b,
    };
    let source_c = CandidateSource {
        markdown: markdown_c,
        document: &document_c,
        sections: &sections_c,
    };
    let candidates = [
        candidate(source_a, "Guide", "Same words", 0, None),
        candidate(source_b, "Guide", "Same words", 1, None),
        candidate(source_c, "Guide", "Unique procedures", 2, None),
    ];

    let result = run_selection(&candidates, &[], 2, u32::MAX);

    assert_eq!(
        result
            .passages
            .iter()
            .map(|passage| passage.revision_id.as_str())
            .collect::<Vec<_>>(),
        [
            document_a.revision_id.as_str(),
            document_c.revision_id.as_str()
        ]
    );
    assert!(result.omissions.evidence);
}

#[test]
fn overlapping_conflict_groups_form_one_selection_unit() {
    let markdown = "# Guide\n\nShared passage.\n";
    let (document_a, sections_a) = prepared(markdown, "a.md");
    let (document_b, sections_b) = prepared(markdown, "b.md");
    let (document_c, sections_c) = prepared(markdown, "c.md");
    let candidates = [
        candidate(
            CandidateSource {
                markdown,
                document: &document_a,
                sections: &sections_a,
            },
            "Guide",
            "Shared passage.",
            0,
            None,
        ),
        candidate(
            CandidateSource {
                markdown,
                document: &document_b,
                sections: &sections_b,
            },
            "Guide",
            "Shared passage.",
            1,
            None,
        ),
        candidate(
            CandidateSource {
                markdown,
                document: &document_c,
                sections: &sections_c,
            },
            "Guide",
            "Shared passage.",
            2,
            None,
        ),
    ];

    let result = run_selection(
        &candidates,
        &[BTreeSet::from([0, 1]), BTreeSet::from([1, 2])],
        3,
        u32::MAX,
    );

    assert_eq!(result.selected_candidates, BTreeSet::from([0, 1, 2]));
    assert_eq!(result.passages.len(), 3);
}

#[test]
fn best_conflict_member_determines_unit_priority() {
    let markdown = "# Guide\n\nShared passage.\n";
    let (document_a, sections_a) = prepared(markdown, "a.md");
    let (document_b, sections_b) = prepared(markdown, "b.md");
    let (document_c, sections_c) = prepared(markdown, "c.md");
    let candidates = [
        candidate(
            CandidateSource {
                markdown,
                document: &document_a,
                sections: &sections_a,
            },
            "Guide",
            "Shared passage.",
            0,
            None,
        ),
        candidate(
            CandidateSource {
                markdown,
                document: &document_b,
                sections: &sections_b,
            },
            "Guide",
            "Shared passage.",
            1,
            None,
        ),
        candidate(
            CandidateSource {
                markdown,
                document: &document_c,
                sections: &sections_c,
            },
            "Guide",
            "Shared passage.",
            2,
            None,
        ),
    ];

    let result = run_selection(&candidates, &[BTreeSet::from([0, 2])], 2, u32::MAX);

    assert_eq!(
        result
            .passages
            .iter()
            .map(|passage| passage.revision_id.as_str())
            .collect::<Vec<_>>(),
        [
            document_a.revision_id.as_str(),
            document_c.revision_id.as_str()
        ]
    );
    assert_eq!(result.selected_candidates, BTreeSet::from([0, 2]));
    assert!(result.omissions.evidence);
    assert!(!result.omissions.conflict);
}

#[test]
fn an_unfitting_first_window_does_not_stop_later_candidates() {
    let large = "L".repeat(1_200);
    let markdown = format!("# Guide\n\n## Large\n\n{large}\n\n## Small\n\nFits.\n");
    let (document, sections) = prepared(&markdown, "two-sections.md");
    let source = CandidateSource {
        markdown: &markdown,
        document: &document,
        sections: &sections,
    };
    let candidates = [
        candidate(source, "Large", &large, 0, None),
        candidate(source, "Small", "Fits.", 1, None),
    ];

    let result = run_selection(&candidates, &[], 2, 500);

    assert_eq!(result.passages.len(), 1);
    assert_eq!(result.passages[0].text, "## Small\n\nFits.\n");
    assert_eq!(result.passages[0].revision_id, document.revision_id);
    assert!(result.omissions.evidence);
}

#[test]
fn mandatory_whole_sibling_window_adds_before_then_stops_at_budget() {
    let markdown = concat!(
        "# Guide\n\n",
        "Farther context before.\n\n",
        "Short context before.\n\n",
        "The selected paragraph stays whole even when its candidate is a small phrase.\n\n",
        "This after-context is deliberately too large for the measured budget.\n"
    );
    let (document, sections) = prepared(markdown, "window.md");
    let source = CandidateSource {
        markdown,
        document: &document,
        sections: &sections,
    };
    let candidate = candidate(source, "Guide", "selected paragraph", 0, None);
    let plan = candidate
        .expansion
        .window_plan(candidate.required_span)
        .unwrap();
    let before = plan.before.get(1).copied().unwrap();
    let expected_span = Span {
        start: before.start,
        end: plan.mandatory.end,
    };
    let expected_text = markdown
        .get(expected_span.start..expected_span.end)
        .unwrap();
    let mut expected = candidate.template.clone();
    expected.n = 1;
    expected.span = expected_span;
    expected.digest = Digest::of(expected_text.as_bytes());
    expected.text = expected_text.to_owned();
    expected.windowed = true;
    let counter = EvidenceCounter::Utf8Bytes;
    let info = counter_info(&counter).unwrap();
    let evidence_bytes = count_passages(&[expected], &counter, &info, 6_000).unwrap();
    let control = control();

    let result = select(
        &[candidate],
        &[],
        &SelectionBudget {
            graph: &LegacyCanonicalGraph,
            parent_chain_order: ParentChainOrder::default(),
            expansion: ExpansionMode::default(),
            max_passages: 1,
            evidence_bytes,
            counter: &counter,
            counter_info: &info,
            control: &control,
        },
    )
    .unwrap();

    assert_eq!(result.passages.len(), 1);
    assert_eq!(result.passages[0].span, expected_span);
    assert_eq!(result.passages[0].text, expected_text);
    assert!(!result.passages[0].text.contains("after-context"));
    assert!(result.passages[0].windowed);
}

#[test]
fn a_failed_near_sibling_does_not_close_a_non_monotonic_farther_window() {
    let markdown = concat!(
        "# Guide\n\n",
        "Far before.\n\n",
        "Near before.\n\n",
        "Selected paragraph.\n\n",
        "Huge after.\n"
    );
    let (document, sections) = prepared(markdown, "non-monotonic-window.md");
    let candidate = candidate(
        CandidateSource {
            markdown,
            document: &document,
            sections: &sections,
        },
        "Guide",
        "Selected paragraph.",
        0,
        None,
    );
    let counter = EvidenceCounter::Exact(Arc::new(NonMonotonicCounter));
    let info = counter_info(&counter).unwrap();
    let control = control();

    let result = select(
        &[candidate],
        &[],
        &SelectionBudget {
            graph: &LegacyCanonicalGraph,
            parent_chain_order: ParentChainOrder::default(),
            expansion: ExpansionMode::default(),
            max_passages: 1,
            evidence_bytes: 2,
            counter: &counter,
            counter_info: &info,
            control: &control,
        },
    )
    .unwrap();

    assert_eq!(result.passages.len(), 1);
    assert!(result.passages[0].text.contains("Far before."));
    assert!(result.passages[0].text.contains("Near before."));
    assert!(!result.passages[0].text.contains("Huge after."));
}

#[test]
fn selection_refuses_an_already_cancelled_control() {
    let markdown = "# Guide\n\nSource passage.\n";
    let (document, sections) = prepared(markdown, "guide.md");
    let candidate = candidate(
        CandidateSource {
            markdown,
            document: &document,
            sections: &sections,
        },
        "Guide",
        "Source passage.",
        0,
        None,
    );
    let counter = EvidenceCounter::Utf8Bytes;
    let info = counter_info(&counter).unwrap();
    let control = ReadControl {
        deadline: Instant::now() + Duration::from_secs(1),
        cancelled: Arc::new(AtomicBool::new(true)),
    };

    assert!(matches!(
        select(
            &[candidate],
            &[],
            &SelectionBudget {
                graph: &LegacyCanonicalGraph,
                parent_chain_order: ParentChainOrder::default(),
                expansion: ExpansionMode::default(),
                max_passages: 1,
                evidence_bytes: u32::MAX,
                counter: &counter,
                counter_info: &info,
                control: &control,
            }
        ),
        Err(EvidenceError::TimedOut)
    ));
}

#[path = "selection_conflict.rs"]
mod selection_conflict;
