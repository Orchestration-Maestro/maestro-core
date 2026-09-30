use super::super::{CounterMode, EvidenceCounter, EvidenceError};
use super::support::assemble_on_stopped_clock;
use super::support::{evidence_input, fixture};
use crate::{
    query::{Family, Identifier},
    search::{EvidenceInput, Route},
};
use maestro_kernel::{evidence::RouteStatus, store::Database, telemetry::stage::Outcome};
use std::{fmt::Write as _, num::NonZeroU32, sync::Arc};

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

#[tokio::test]
async fn accepts_handoff_fields_at_their_exact_text_and_candidate_limits() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nA source passage.\n")]);
    let mut input = evidence_input(&fixture, "What does the guide document say?");
    input.query = "q".repeat(8192);
    input.understood.normalized.clone_from(&input.query);
    input.version = Some("v".repeat(8192));
    let ranked = input.ranked[0].clone();
    input.ranked = (0..120)
        .map(|index| {
            let mut candidate = ranked.clone();
            candidate.candidate.fused.chunk_id = format!("unmatched-{index}");
            candidate
        })
        .collect();
    input.budget.evidence_bytes = 24_000;
    input.understood.identifiers = (0..64)
        .map(|index| Identifier {
            family: Family::ErrorCode,
            text: format!("ERR_{index}"),
        })
        .collect();

    let bundle = assemble_on_stopped_clock(
        Arc::new(fixture.database),
        input,
        EvidenceCounter::Utf8Bytes,
    )
    .await
    .unwrap();

    assert!(bundle.passages.is_empty());
    assert_eq!(bundle.budget.limit, 24_000);
}

#[tokio::test]
async fn a_24000_byte_answer_bound_budget_delivers_past_the_former_12000_byte_wire() {
    let mut guide = "# Guide\n\n".to_owned();
    for step in 1..=8 {
        write!(guide, "## Step {step}\n\n").unwrap();
        for index in 0..40 {
            write!(
                guide,
                "Step {step} sets agent option o{step}x{index} to {index}. "
            )
            .unwrap();
        }
        guide.push_str("\n\n");
    }
    let fixture = fixture(&[("guide.md", &guide)]);
    let mut input = evidence_input(&fixture, "How are the agent options set?");
    input.evidence.evidence_counter = CounterMode::Utf8AnswerBound;
    input.budget.evidence_bytes = 24_000;

    let bundle = assemble_on_stopped_clock(
        Arc::new(fixture.database),
        input,
        EvidenceCounter::AnswerBoundUtf8Bytes,
    )
    .await
    .unwrap();

    let wire = serde_json::to_string(&bundle.passages).unwrap().len();
    assert!(wire > 12_000, "{wire}");
    assert!(bundle.budget.evidence_bytes > 12_000, "{:?}", bundle.budget);
    assert_eq!(bundle.budget.limit, 24_000);
}

#[tokio::test]
async fn rejects_each_invalid_handoff_field_at_the_public_entry_point() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nA source passage.\n")]);
    let base = evidence_input(&fixture, "What does the guide document say?");
    let database = Arc::new(fixture.database);
    assemble_on_stopped_clock(database.clone(), base.clone(), EvidenceCounter::Utf8Bytes)
        .await
        .expect("the control handoff is valid");

    for (name, change, expected_reason) in invalid_cases() {
        assert_invalid_case(&database, &base, name, change, expected_reason).await;
    }
}

#[tokio::test]
async fn refuses_a_counter_other_than_the_configured_evidence_counter() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nA source passage.\n")]);
    let base = evidence_input(&fixture, "What does the guide document say?");
    let database = Arc::new(fixture.database);
    let mismatches = [
        (CounterMode::Utf8AnswerBound, EvidenceCounter::Utf8Bytes),
        (CounterMode::Exact, EvidenceCounter::Utf8Bytes),
        (CounterMode::Utf8, EvidenceCounter::AnswerBoundUtf8Bytes),
    ];
    for (setting, counter) in mismatches {
        let mut input = base.clone();
        input.evidence.evidence_counter = setting;
        let error = assemble_on_stopped_clock(database.clone(), input, counter)
            .await
            .unwrap_err();
        assert!(
            matches!(
                &error,
                EvidenceError::InvalidRequest(reason)
                    if reason == "evidence counter differs from the configured evidence_counter"
            ),
            "{setting:?}: {error:?}"
        );
    }
    let mut input = base;
    input.evidence.evidence_counter = CounterMode::Utf8AnswerBound;
    assemble_on_stopped_clock(database, input, EvidenceCounter::AnswerBoundUtf8Bytes)
        .await
        .expect("the configured counter is accepted");
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
    let error = assemble_on_stopped_clock(database.clone(), input, EvidenceCounter::Utf8Bytes)
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
            |input| input.budget.evidence_bytes = 0,
            BOUNDS_ERROR,
        ),
        (
            "token budget over 24000",
            |input| input.budget.evidence_bytes = 24_001,
            BOUNDS_ERROR,
        ),
        (
            "zero deadline budget",
            |input| input.budget.deadline_ms = 0,
            BOUNDS_ERROR,
        ),
        (
            "deadline budget over 30000",
            |input| input.budget.deadline_ms = 30001,
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

#[test]
fn an_assembly_refused_by_its_contract_or_rights_is_refused_not_failed() {
    let errors = [
        (
            EvidenceError::InvalidRequest("k".to_owned()),
            Outcome::Refused,
        ),
        (EvidenceError::NotVisible, Outcome::Refused),
        (EvidenceError::PermissionsChanged, Outcome::Refused),
        (EvidenceError::TimedOut, Outcome::Timeout),
        (EvidenceError::Integrity("span".to_owned()), Outcome::Error),
        (EvidenceError::WorkerFailed, Outcome::Error),
    ];
    for (error, outcome) in errors {
        assert_eq!(error.outcome(), outcome, "{error}");
    }
}
