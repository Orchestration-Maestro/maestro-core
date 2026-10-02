use super::support::assemble_on_stopped_clock;
use super::{
    super::{EvidenceCounter, EvidenceSettings},
    support::{Fixture, evidence_input, fixture},
};
use crate::{
    prepare::tests::scratch::{accept_with_warnings, revision_of},
    query::Language,
    search::EvidenceInput,
};
use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::{
    artifact::Digest,
    evidence::{Budget, Bundle, Passage, RouteStatus, Schema, Span, Trace},
};
use std::{collections::BTreeSet, slice::from_ref, sync::Arc};

#[tokio::test]
async fn public_entry_emits_every_bundle_field_deterministically() {
    let fixture = fixture(&[(
        "guide.md",
        "# Guide\n\nThe canonical source text is authoritative.\n",
    )]);
    let mut input = evidence_input(&fixture, "What does the guide document say?");
    input.understood.language = Language::English;
    let expected = expected_bundle(&fixture, &input);
    let database = Arc::new(fixture.database);

    let bundle =
        assemble_on_stopped_clock(database.clone(), input.clone(), EvidenceCounter::Utf8Bytes)
            .await
            .unwrap();
    let repeated = assemble_on_stopped_clock(
        database,
        input,
        EvidenceSettings::default().counter().unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(bundle, expected);
    assert_eq!(
        serde_json::to_vec(&bundle).unwrap(),
        serde_json::to_vec(&repeated).unwrap()
    );
}

fn expected_bundle(fixture: &Fixture, input: &EvidenceInput) -> Bundle {
    let revision_id = revision_of(&fixture.database, &fixture.scopes, "guide.md");
    let revision = fixture
        .database
        .revision(&fixture.scopes, &revision_id)
        .unwrap()
        .unwrap();
    let document = fixture
        .database
        .document(&fixture.scopes, &revision.document_id)
        .unwrap()
        .unwrap();
    let markdown =
        String::from_utf8(fixture.database.get(&revision.original_digest).unwrap()).unwrap();
    let canonical: CanonicalDocument =
        serde_json::from_slice(&fixture.database.get(&revision.canonical_digest).unwrap()).unwrap();
    let section = canonical.sections.first().unwrap();
    let chunk_ids = input
        .ranked
        .iter()
        .map(|ranked| ranked.candidate.fused.chunk_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let passage = Passage {
        n: 1,
        section_id: Some(section.section_id.clone()),
        document_id: document.id,
        revision_id: revision.id,
        title: canonical.source_metadata.title.clone().unwrap_or_default(),
        section_path: section.heading_path.clone(),
        version: None,
        source_ref: document.source_ref,
        span: Span {
            start: 0,
            end: markdown.len(),
        },
        digest: Digest::of(markdown.as_bytes()),
        text: markdown,
        windowed: false,
        alternates: Vec::new(),
    };
    let evidence_bytes =
        u32::try_from(serde_json::to_vec(from_ref(&passage)).unwrap().len()).unwrap();
    Bundle {
        schema: Schema::V1,
        collection: input.generation.collection_id.clone(),
        generation: input.generation.id,
        query: input.query.clone(),
        lang: "en".to_owned(),
        routes: input.routes.clone(),
        passages: vec![passage],
        conflicts: Vec::new(),
        known_gaps: Vec::new(),
        budget: Budget {
            evidence_bytes,
            limit: input.budget.evidence_bytes,
            counter: Some("evidence-utf8-bytes/1".to_owned()),
            estimated: true,
        },
        request_budget: Some(input.budget),
        inventory: None,
        trace: vec![Trace {
            parent_context_of: Vec::new(),
            n: 1,
            score: Some(0.8),
            routes: vec!["lexical".to_owned()],
            chunk_ids,
            procedural: false,
        }],
    }
}

#[tokio::test]
async fn language_tags_and_unavailable_reranking_are_preserved() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nAn authoritative passage.\n")]);
    let language_inputs = [
        (Language::French, "fr"),
        (Language::English, "en"),
        (Language::Unknown, "und"),
    ]
    .map(|(language, expected)| {
        let mut input = evidence_input(&fixture, "A general question about the guide.");
        input.understood.language = language;
        (input, expected)
    });
    let mut rerank_input = evidence_input(&fixture, "A general question about the guide.");
    rerank_input.routes.insert(
        "rerank".to_owned(),
        RouteStatus::Unavailable("model offline".to_owned()),
    );
    for ranked in &mut rerank_input.ranked {
        ranked.score = None;
    }
    let database = Arc::new(fixture.database);
    for (input, expected) in language_inputs {
        let bundle = assemble_on_stopped_clock(database.clone(), input, EvidenceCounter::Utf8Bytes)
            .await
            .unwrap();
        assert_eq!(bundle.lang, expected);
    }

    let bundle = assemble_on_stopped_clock(database, rerank_input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap();
    assert_eq!(
        bundle.routes.get("rerank"),
        Some(&RouteStatus::Unavailable("model offline".to_owned()))
    );
    assert!(bundle.trace.iter().all(|trace| trace.score.is_none()));
}

#[tokio::test]
async fn identifier_route_can_be_unavailable_while_lexical_evidence_remains() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nThe guide describes ERR_404.\n")]);
    let mut input = evidence_input(&fixture, "How can I resolve ERR_404?");
    input.routes.insert(
        "identifier".to_owned(),
        RouteStatus::Unavailable("identifier index warming".to_owned()),
    );

    let bundle = assemble_on_stopped_clock(
        Arc::new(fixture.database),
        input,
        EvidenceCounter::Utf8Bytes,
    )
    .await
    .unwrap();

    assert_eq!(
        bundle.routes.get("identifier"),
        Some(&RouteStatus::Unavailable(
            "identifier index warming".to_owned()
        ))
    );
    assert!(bundle.trace.iter().all(|trace| trace.routes == ["lexical"]));
    assert!(bundle.known_gaps.is_empty());
}

#[tokio::test]
async fn unresolved_candidates_and_budgeted_identifiers_have_exact_gaps() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nThe guide describes ERR_404.\n")]);
    let mut unresolved_input = evidence_input(&fixture, "What does the guide say?");
    let mut first = unresolved_input.ranked.first().unwrap().clone();
    first.candidate.fused.chunk_id = "unknown-chunk".to_owned();
    unresolved_input.ranked = vec![first];
    let mut budgeted_input = evidence_input(&fixture, "How can I fix ERR_404?");
    budgeted_input.budget.k = 1;
    budgeted_input.budget.evidence_bytes = 1;
    let database = Arc::new(fixture.database);

    let unresolved = assemble_on_stopped_clock(
        database.clone(),
        unresolved_input,
        EvidenceCounter::Utf8Bytes,
    )
    .await
    .unwrap();
    assert!(unresolved.passages.is_empty());
    assert_eq!(
        unresolved.known_gaps,
        [
            "No accessible evidence was returned for this query in the pinned generation.",
            "One or more candidate passages could not be resolved in the current scopes.",
        ]
    );

    let budgeted = assemble_on_stopped_clock(database, budgeted_input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap();
    assert!(budgeted.passages.is_empty());
    assert_eq!(
        budgeted.known_gaps,
        [
            "No accessible evidence was returned for this query in the pinned generation.",
            concat!(
                "Required identifier \"ERR_404\" was found in candidates ",
                "but is absent after budgeting."
            ),
            "Some candidate evidence was omitted by the evidence or passage budget.",
        ]
    );
}

#[tokio::test]
async fn accepted_warning_dispositions_are_visible_in_exact_source_gaps() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nA source with reviewed warnings.\n")]);
    let revision_id = revision_of(&fixture.database, &fixture.scopes, "guide.md");
    accept_with_warnings(&fixture.scratch, &revision_id);
    let input = evidence_input(&fixture, "What does the guide say?");

    let bundle = assemble_on_stopped_clock(
        Arc::new(fixture.database),
        input,
        EvidenceCounter::Utf8Bytes,
    )
    .await
    .unwrap();

    assert_eq!(
        bundle.known_gaps,
        [
            "Passage 1 source warning: \"approved warning\"",
            "Passage 1 source warning: test.warning",
            "Passage 1 source warning: missing_metadata",
        ]
    );
}
