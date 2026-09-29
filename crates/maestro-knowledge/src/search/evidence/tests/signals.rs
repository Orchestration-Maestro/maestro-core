use super::super::signals::{
    GapInput, OmissionStatus, PassageOrder, SourceWarning, TraceInput, WithinPassageConflict,
    append_distinct_gaps, build_known_gaps, order_passages, trace_for_passage,
};
use super::super::spans::SeedSpan;
use crate::search::{Route, evidence::delivery_graph::PrimaryContribution};
use maestro_canonicalization::{CanonicalDocument, CanonicalizeInput, canonicalize};
use maestro_kernel::artifact::Digest;
use maestro_kernel::evidence::{Alternate, Passage, Span};

fn document(markdown: &str) -> CanonicalDocument {
    canonicalize(CanonicalizeInput::new(markdown, "fixture.md")).unwrap()
}

fn passage(document_id: &str, revision_id: &str, start: usize) -> Passage {
    Passage {
        n: 99,
        section_id: None,
        document_id: document_id.to_owned(),
        revision_id: revision_id.to_owned(),
        title: String::new(),
        section_path: Vec::new(),
        version: None,
        source_ref: String::new(),
        span: Span {
            start,
            end: start + 1,
        },
        digest: Digest::of(b"x"),
        text: "x".to_owned(),
        windowed: false,
        alternates: Vec::<Alternate>::new(),
    }
}

fn seed(
    chunk_id: &str,
    revision_id: &str,
    span: Span,
    score: Option<f64>,
    routes: &[Route],
) -> SeedSpan {
    SeedSpan {
        chunk_id: chunk_id.to_owned(),
        revision_id: revision_id.to_owned(),
        section_id: None,
        span,
        input_position: 0,
        score,
        routes: routes.iter().copied().collect(),
    }
}

#[test]
fn reading_order_groups_by_best_rank_then_sorts_source_spans() {
    let passages = order_passages(vec![
        PassageOrder {
            passage: passage("doc-z", "rev-z", 100),
            input_position: 10,
        },
        PassageOrder {
            passage: passage("doc-z", "rev-z", 1),
            input_position: 2,
        },
        PassageOrder {
            passage: passage("doc-b", "rev-b", 40),
            input_position: 2,
        },
        PassageOrder {
            passage: passage("doc-a", "rev-a", 50),
            input_position: 1,
        },
    ])
    .unwrap();

    assert_eq!(
        passages
            .iter()
            .map(|passage| (passage.document_id.as_str(), passage.span.start, passage.n))
            .collect::<Vec<_>>(),
        [
            ("doc-a", 50, 1),
            ("doc-b", 40, 2),
            ("doc-z", 1, 3),
            ("doc-z", 100, 4)
        ]
    );
}

#[test]
fn trace_keeps_sorted_unique_source_chunks_max_finite_score_and_ordered_routes() {
    let markdown = "1. First step\n2. Second step\n";
    let document = document(markdown);
    let span = Span {
        start: 0,
        end: markdown.len(),
    };
    let seeds = vec![
        seed(
            "chunk-b",
            "rev-a",
            Span { start: 3, end: 8 },
            Some(0.4),
            &[Route::Dense],
        ),
        seed(
            "chunk-a",
            "rev-a",
            Span { start: 17, end: 28 },
            Some(0.9),
            &[Route::Lexical],
        ),
        seed(
            "chunk-a",
            "rev-a",
            Span { start: 3, end: 8 },
            Some(0.6),
            &[Route::Dense],
        ),
        seed(
            "other-revision",
            "rev-b",
            Span { start: 0, end: 4 },
            Some(1.0),
            &[Route::Dense],
        ),
    ];
    let trace = trace_for_passage(&TraceInput {
        primary: &[],
        parent_context_of: &[],
        number: 1,
        revision_id: "rev-a",
        span,
        seeds: &seeds,
        document: &document,
        markdown,
    })
    .unwrap();

    assert_eq!(trace.chunk_ids, ["chunk-a", "chunk-b"]);
    assert_eq!(trace.routes, ["dense", "lexical"]);
    assert_eq!(trace.score, Some(0.9));
    assert!(trace.procedural);
}

#[test]
fn trace_refuses_empty_seed_spans() {
    let markdown = "A paragraph.\n";
    let document = document(markdown);
    let seeds = [seed(
        "chunk-a",
        "rev-a",
        Span { start: 0, end: 0 },
        Some(0.5),
        &[],
    )];

    assert_eq!(
        trace_for_passage(&TraceInput {
            primary: &[],
            parent_context_of: &[],
            number: 1,
            revision_id: "rev-a",
            span: Span {
                start: 0,
                end: markdown.len(),
            },
            seeds: &seeds,
            document: &document,
            markdown,
        })
        .unwrap_err(),
        "candidate seed span is invalid"
    );
}

#[test]
fn trace_refuses_out_of_range_passage_spans() {
    let markdown = "A paragraph.\n";
    let document = document(markdown);
    let seeds = [seed(
        "chunk-a",
        "rev-a",
        Span { start: 0, end: 1 },
        Some(0.5),
        &[],
    )];

    assert_eq!(
        trace_for_passage(&TraceInput {
            primary: &[],
            parent_context_of: &[],
            number: 1,
            revision_id: "rev-a",
            span: Span {
                start: 0,
                end: usize::MAX,
            },
            seeds: &seeds,
            document: &document,
            markdown,
        })
        .unwrap_err(),
        "passage span is invalid"
    );
}

#[test]
fn a_partial_ordered_list_is_not_marked_procedural() {
    let markdown = "1. First step\n2. Second step\n";
    let document = document(markdown);
    let start = markdown.find("First step").unwrap();
    let span = Span {
        start,
        end: start + "First step".len(),
    };
    let seeds = [seed("chunk-a", "rev-a", span, Some(0.5), &[])];
    let trace = trace_for_passage(&TraceInput {
        primary: &[],
        parent_context_of: &[],
        number: 1,
        revision_id: "rev-a",
        span,
        seeds: &seeds,
        document: &document,
        markdown,
    })
    .unwrap();

    assert!(!trace.procedural);
}

#[test]
fn ordinary_prose_is_not_marked_procedural() {
    let markdown = "A paragraph.\n";
    let document = document(markdown);
    let seeds = [seed(
        "chunk-a",
        "rev-a",
        Span {
            start: 0,
            end: markdown.len(),
        },
        Some(0.5),
        &[],
    )];
    let trace = trace_for_passage(&TraceInput {
        primary: &[],
        parent_context_of: &[],
        number: 1,
        revision_id: "rev-a",
        span: Span {
            start: 0,
            end: markdown.len(),
        },
        seeds: &seeds,
        document: &document,
        markdown,
    })
    .unwrap();

    assert!(!trace.procedural);
}

#[test]
fn trace_refuses_a_passage_without_primary_chunk_references() {
    let markdown = "A paragraph.\n";
    let document = document(markdown);
    assert!(
        trace_for_passage(&TraceInput {
            primary: &[],
            parent_context_of: &[],
            number: 1,
            revision_id: "rev-a",
            span: Span {
                start: 0,
                end: markdown.len()
            },
            seeds: &[],
            document: &document,
            markdown,
        })
        .is_err()
    );
}

#[test]
fn computed_gaps_append_once_without_reordering_inherited_entries() {
    let mut gaps = vec!["inherited".to_owned(), "inherited".to_owned()];
    append_distinct_gaps(
        &mut gaps,
        ["inherited", "new", "new", "another"].map(str::to_owned),
    );
    assert_eq!(gaps, ["inherited", "inherited", "new", "another"]);
}

#[test]
fn table_prefixes_the_budget_omitted_are_counted_in_one_gap() {
    let gaps = build_known_gaps(GapInput {
        omissions: OmissionStatus {
            table_prefix_omissions: 2,
            ..OmissionStatus::default()
        },
        ..GapInput::default()
    })
    .unwrap();
    assert!(
        gaps.contains(
            &"Table-prefix candidates omitted by the evidence or passage budget: 2.".to_owned()
        ),
        "{gaps:?}"
    );
}

#[test]
fn known_gaps_use_inventory_wording_and_exact_identifier_boundaries() {
    assert_eq!(
        build_known_gaps(GapInput::default()).unwrap(),
        ["No accessible evidence was returned for this query in the pinned generation."]
    );
    assert_eq!(
        build_known_gaps(GapInput {
            has_inventory: true,
            ..GapInput::default()
        })
        .unwrap(),
        [concat!(
            "No source passages were returned; inventory totals are ",
            "independent of supporting passages."
        )]
    );

    let mut returned = passage("doc-a", "rev-a", 0);
    returned.n = 1;
    returned.text = "The failing code is ERR_70.".to_owned();
    returned.version = None;
    returned.windowed = true;
    let passages = [returned];
    let identifiers = [
        "missing".to_owned(),
        "ERR_70".to_owned(),
        "ERR_7".to_owned(),
    ];
    let candidates = ["Earlier candidate used ERR_7.".to_owned()];
    let reasons = [r#"warning "quoted""#.to_owned()];
    let rule_ids = ["rule-1".to_owned()];
    let warning_codes = ["canonical-warning".to_owned()];
    let conflicts = [WithinPassageConflict {
        passage_number: 1,
        entity: "Agent",
        attribute: "Port",
    }];
    let warnings = [SourceWarning {
        passage_number: 1,
        disposition_reasons: &reasons,
        rule_ids: &rule_ids,
        warning_codes: &warning_codes,
    }];

    assert_eq!(
        build_known_gaps(GapInput {
            inherited: vec!["caller note".to_owned()],
            passages: &passages,
            unresolved_candidate: true,
            identifiers: &identifiers,
            candidate_texts: &candidates,
            requested_version: Some("2"),
            latest_undetermined: true,
            omissions: OmissionStatus {
                table_prefix_omissions: 0,
                evidence: true,
                conflict: true,
            },
            within_passage_conflicts: &conflicts,
            source_warnings: &warnings,
            ..GapInput::default()
        })
        .unwrap(),
        [
            "caller note",
            "One or more candidate passages could not be resolved in the current scopes.",
            "Required identifier \"ERR_7\" was found in candidates but is absent after budgeting.",
            "Required identifier \"missing\" is absent from returned passages.",
            "Requested version \"2\" is not represented in returned passages.",
            "Latest version could not be determined for one or more candidate families.",
            "Passage 1 is a window; surrounding section text is omitted.",
            "Some candidate evidence was omitted by the evidence or passage budget.",
            concat!(
                "Conflicting values could not be retained together within the ",
                "evidence or passage budget."
            ),
            "Passage 1 contains different explicit values for \"Agent\" / \"Port\".",
            r#"Passage 1 source warning: "warning \"quoted\"""#,
            "Passage 1 source warning: rule-1",
            "Passage 1 source warning: canonical-warning",
        ]
    );
}

#[test]
fn a_primary_part_from_another_chunk_does_not_contain_this_seed() {
    let markdown = "A source passage.";
    let document = document(markdown);
    let passage_span = Span { start: 2, end: 8 };
    let seeds = [seed(
        "seed-a",
        "rev-a",
        Span { start: 0, end: 10 },
        Some(0.8),
        &[Route::Lexical],
    )];
    let primary = [PrimaryContribution {
        chunk_id: "seed-b".to_owned(),
        span: passage_span,
    }];
    let parent_context_of = ["seed-b".to_owned()];
    let trace = trace_for_passage(&TraceInput {
        primary: &primary,
        parent_context_of: &parent_context_of,
        number: 1,
        revision_id: "rev-a",
        span: passage_span,
        seeds: &seeds,
        document: &document,
        markdown,
    })
    .unwrap();

    assert!(trace.chunk_ids.is_empty());
}

#[test]
fn a_seed_outside_the_passage_is_not_attributed_to_another_primary_part() {
    let markdown = "A source passage.";
    let document = document(markdown);
    let passage_span = Span { start: 4, end: 8 };
    let seeds = [seed(
        "seed-a",
        "rev-a",
        Span { start: 0, end: 6 },
        Some(0.8),
        &[Route::Lexical],
    )];
    let primary = [PrimaryContribution {
        chunk_id: "seed-a".to_owned(),
        span: Span { start: 2, end: 3 },
    }];
    let parent_context_of = ["parent".to_owned()];
    let trace = trace_for_passage(&TraceInput {
        primary: &primary,
        parent_context_of: &parent_context_of,
        number: 1,
        revision_id: "rev-a",
        span: passage_span,
        seeds: &seeds,
        document: &document,
        markdown,
    })
    .unwrap();

    assert!(trace.chunk_ids.is_empty());
}

#[test]
fn an_unrelated_seed_does_not_contribute_score_or_routes() {
    let markdown = "A source passage.";
    let document = document(markdown);
    let passage_span = Span { start: 0, end: 1 };
    let seeds = [seed(
        "seed-a",
        "rev-a",
        Span { start: 2, end: 3 },
        Some(0.8),
        &[Route::Lexical],
    )];
    let parent_context_of = ["parent".to_owned()];
    let trace = trace_for_passage(&TraceInput {
        primary: &[],
        parent_context_of: &parent_context_of,
        number: 1,
        revision_id: "rev-a",
        span: passage_span,
        seeds: &seeds,
        document: &document,
        markdown,
    })
    .unwrap();

    assert_eq!(trace.score, None);
    assert!(trace.routes.is_empty());
}
