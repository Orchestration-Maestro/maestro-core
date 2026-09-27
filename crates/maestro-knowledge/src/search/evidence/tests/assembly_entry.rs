use super::super::{EvidenceCounter, EvidenceError, assemble_evidence};
use super::support::{evidence_input, fixture};
use crate::{
    query::{Family, Identifier},
    search::{EvidenceInput, Route},
};
use maestro_kernel::{evidence::RouteStatus, store::Database};
use std::{num::NonZeroU32, sync::Arc};

type InvalidCase = (&'static str, fn(&mut EvidenceInput), &'static str);

const TEXT_ERROR: &str = "evidence handoff text or identity is invalid";
const BOUNDS_ERROR: &str = "evidence handoff exceeds its bounded request limits";
const IDENTIFIERS_ERROR: &str = "evidence handoff has too many or blank identifiers";
const RERANK_ERROR: &str = "evidence handoff has no rerank status";
const ROUTE_REASON_ERROR: &str = "evidence handoff has a blank unavailable reason";
const CANDIDATE_ERROR: &str = "evidence handoff candidate metadata is invalid";
const RERANK_SCORE_ERROR: &str = "unavailable reranking cannot supply candidate scores";
const MISSING_ROUTE_ERROR: &str = "candidate route status is missing";
const UNAVAILABLE_ROUTE_ERROR: &str = "an unavailable route supplied a candidate";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejects_each_invalid_handoff_field_at_the_public_entry_point() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nA source passage.\n")]);
    let base = evidence_input(&fixture, "What does the guide document say?");
    let database = Arc::new(fixture.database);
    assemble_evidence(database.clone(), base.clone(), EvidenceCounter::Utf8Bytes)
        .await
        .expect("the control handoff is valid");

    for (name, change, expected_reason) in invalid_cases() {
        assert_invalid_case(&database, &base, name, change, expected_reason).await;
    }
}

async fn assert_invalid_case(
    database: &Arc<Database>,
    base: &EvidenceInput,
    name: &str,
    change: fn(&mut EvidenceInput),
    expected_reason: &str,
) {
    let mut input = base.clone();
    change(&mut input);
    let error = assemble_evidence(database.clone(), input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap_err();
    match error {
        EvidenceError::InvalidRequest(reason) => assert_eq!(reason, expected_reason, "{name}"),
        other => panic!("{name}: expected InvalidRequest, got {other:?}"),
    }
}

fn invalid_cases() -> Vec<InvalidCase> {
    let mut cases = request_identity_cases();
    cases.extend(request_bound_cases());
    cases.extend(candidate_metadata_cases());
    cases.extend(route_status_cases());
    cases
}

fn request_identity_cases() -> Vec<InvalidCase> {
    vec![
        ("blank query", |input| input.query.clear(), TEXT_ERROR),
        (
            "blank normalized query",
            |input| input.understood.normalized.clear(),
            TEXT_ERROR,
        ),
        (
            "blank principal",
            |input| input.principal.clear(),
            TEXT_ERROR,
        ),
        (
            "query over 8192 bytes",
            |input| {
                input.query = "x".repeat(8193);
                input.understood.normalized.clone_from(&input.query);
            },
            TEXT_ERROR,
        ),
        (
            "empty explicit version",
            |input| input.version = Some(String::new()),
            TEXT_ERROR,
        ),
        (
            "explicit version over 8192 bytes",
            |input| input.version = Some("v".repeat(8193)),
            TEXT_ERROR,
        ),
        (
            "query and understanding differ",
            |input| input.understood.normalized = "different".to_owned(),
            TEXT_ERROR,
        ),
        (
            "more than 64 identifiers",
            |input| {
                input.understood.identifiers = (0..65)
                    .map(|index| Identifier {
                        family: Family::ErrorCode,
                        text: format!("ERR_{index}"),
                    })
                    .collect();
            },
            IDENTIFIERS_ERROR,
        ),
        (
            "blank identifier",
            |input| {
                input.understood.identifiers.push(Identifier {
                    family: Family::ErrorCode,
                    text: " ".to_owned(),
                });
            },
            IDENTIFIERS_ERROR,
        ),
    ]
}

fn request_bound_cases() -> Vec<InvalidCase> {
    vec![
        (
            "zero passage budget",
            |input| input.budget.k = 0,
            BOUNDS_ERROR,
        ),
        (
            "passage budget over 50",
            |input| input.budget.k = 51,
            BOUNDS_ERROR,
        ),
        (
            "zero token budget",
            |input| input.budget.max_tokens = 0,
            BOUNDS_ERROR,
        ),
        (
            "token budget over 12000",
            |input| input.budget.max_tokens = 12001,
            BOUNDS_ERROR,
        ),
        (
            "zero deadline budget",
            |input| input.budget.deadline_ms = 0,
            BOUNDS_ERROR,
        ),
        (
            "deadline budget over 10000",
            |input| input.budget.deadline_ms = 10001,
            BOUNDS_ERROR,
        ),
        (
            "more than 120 candidates",
            |input| {
                let first = input.ranked[0].clone();
                input.ranked = vec![first; 121];
            },
            BOUNDS_ERROR,
        ),
    ]
}

fn candidate_metadata_cases() -> Vec<InvalidCase> {
    vec![
        (
            "blank candidate ID",
            |input| input.ranked[0].candidate.fused.chunk_id = " ".to_owned(),
            CANDIDATE_ERROR,
        ),
        (
            "duplicate candidate ID",
            |input| input.ranked.push(input.ranked[0].clone()),
            CANDIDATE_ERROR,
        ),
        (
            "non-finite fusion score",
            |input| input.ranked[0].candidate.fused.score = f64::INFINITY,
            CANDIDATE_ERROR,
        ),
        (
            "non-finite rerank score",
            |input| input.ranked[0].score = Some(f64::NAN),
            CANDIDATE_ERROR,
        ),
        (
            "candidate has no contributing routes",
            |input| input.ranked[0].candidate.fused.ranks.clear(),
            CANDIDATE_ERROR,
        ),
    ]
}

fn route_status_cases() -> Vec<InvalidCase> {
    vec![
        (
            "missing rerank status",
            |input| {
                input.routes.remove("rerank");
            },
            RERANK_ERROR,
        ),
        (
            "blank unavailable route reason",
            |input| {
                input.routes.insert(
                    "structured".to_owned(),
                    RouteStatus::Unavailable(" ".to_owned()),
                );
            },
            ROUTE_REASON_ERROR,
        ),
        (
            "unavailable rerank supplies a score",
            |input| {
                input.routes.insert(
                    "rerank".to_owned(),
                    RouteStatus::Unavailable("offline".to_owned()),
                );
            },
            RERANK_SCORE_ERROR,
        ),
        (
            "candidate route has no status",
            |input| {
                input.ranked[0].candidate.fused.ranks.clear();
                input.ranked[0]
                    .candidate
                    .fused
                    .ranks
                    .insert(Route::Identifier, NonZeroU32::new(1).unwrap());
            },
            MISSING_ROUTE_ERROR,
        ),
        (
            "unavailable retrieval route supplied a candidate",
            |input| {
                input.routes.insert(
                    "lexical".to_owned(),
                    RouteStatus::Unavailable("offline".to_owned()),
                );
            },
            UNAVAILABLE_ROUTE_ERROR,
        ),
    ]
}
