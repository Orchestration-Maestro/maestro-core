//! Tests of what the ladder's rows may not hide: answerable questions left
//! unanswered, refusals that are failures, and rows missing, repeated or
//! unknown to the suite.

use super::ladder::{REFUSED, answered, ask_ending, count, floor, passing, right_section, score};
use crate::{
    answer::RefusalCode,
    eval::{AskOutcome, Floor, FloorStatus, Measure, SectionRef},
};

#[test]
fn refusing_all_but_one_answerable_question_fails() {
    let mut questions = passing();
    for question in &mut questions[1..84] {
        question.ask = ask_ending(REFUSED);
    }
    let refused = score(&questions);
    for question in &mut questions[1..84] {
        question.ask = ask_ending(AskOutcome::Failed);
    }
    let failed = score(&questions);

    assert_eq!(count(&refused, Floor::Citation), (1, 1, FloorStatus::Pass));
    assert_eq!(count(&refused, Floor::Answered), (1, 84, FloorStatus::Fail));
    assert!(!refused.passed);
    assert_eq!(count(&failed, Floor::Answered), (1, 84, FloorStatus::Fail));
    assert_eq!((failed.failed_asks, failed.false_refusals), (83, 0));
    assert!(!failed.passed);
}

#[test]
fn answered_needs_68_of_84_with_a_right_citation() {
    let mut questions = passing();
    for question in &mut questions[..16] {
        question.ask = ask_ending(AskOutcome::TimedOut);
    }
    let at_68 = score(&questions);
    questions[16].ask = answered(vec![SectionRef::section("wrong", "s1")], 0);
    let at_67 = score(&questions);

    assert_eq!(count(&at_68, Floor::Answered), (68, 84, FloorStatus::Pass));
    assert!(matches!(
        floor(&at_68, Floor::Answered).measure,
        Measure::Share {
            percent: 80,
            required: 68,
            ..
        }
    ));
    assert_eq!(count(&at_67, Floor::Answered), (67, 84, FloorStatus::Fail));
    assert!(
        at_67
            .to_markdown()
            .contains("| Answerable answered right | 67/84 (79.8%) | >= 80% (68/84) | FAIL |\n")
    );
}

#[test]
fn not_found_no_evidence_and_unsupported_are_refusals() {
    let mut questions = passing();
    questions[84].ask = ask_ending(AskOutcome::Refused(RefusalCode::NoEvidence));
    questions[85].ask = ask_ending(AskOutcome::Refused(RefusalCode::Unsupported));
    questions[0].ask = ask_ending(AskOutcome::Refused(RefusalCode::NoEvidence));
    questions[1].ask = ask_ending(AskOutcome::Refused(RefusalCode::Unsupported));
    let score = score(&questions);

    assert_eq!(count(&score, Floor::Refused), (16, 16, FloorStatus::Pass));
    assert_eq!((score.false_refusals, score.failed_asks), (2, 0));
}

#[test]
fn an_unavailable_answerer_is_a_failure_not_a_refusal() {
    let unavailable = AskOutcome::Refused(RefusalCode::AnswererUnavailable);
    let mut questions = passing();
    for question in &mut questions {
        question.ask = ask_ending(unavailable.clone());
    }
    let score = score(&questions);

    assert_eq!(count(&score, Floor::Refused), (0, 16, FloorStatus::Fail));
    assert_eq!((score.false_refusals, score.failed_asks), (0, 100));
    assert!(!score.passed);
}

#[test]
fn a_missing_row_fails_against_the_suite_denominators() {
    let mut questions = passing();
    questions.remove(0);
    let score = score(&questions);

    assert_eq!(count(&score, Floor::Top10), (83, 84, FloorStatus::Pass));
    assert_eq!(count(&score, Floor::Top1), (83, 84, FloorStatus::Pass));
    assert_eq!(count(&score, Floor::Answered), (83, 84, FloorStatus::Pass));
    assert!(matches!(
        floor(&score, Floor::Top10).measure,
        Measure::Share { required: 76, .. }
    ));
    assert!(matches!(
        floor(&score, Floor::SearchP95).measure,
        Measure::Latency {
            p95_us: Some(100_000),
            timeouts: 0,
            of: 100,
            ..
        }
    ));
    assert_eq!(
        (score.missing, score.failed_searches, score.failed_asks),
        (1, 1, 1)
    );
    assert!(score.rejected_ids.is_empty());
    assert!(score.passed);
}

#[test]
fn a_repeated_or_unknown_row_makes_every_floor_unavailable() {
    let mut repeated = passing();
    repeated.push(repeated[3].clone());
    let repeated = score(&repeated);
    let mut unknown = passing();
    unknown[99].id = "stranger".to_owned();
    let unknown = score(&unknown);

    assert!(
        repeated
            .floors
            .iter()
            .all(|result| result.status == FloorStatus::Unavailable)
    );
    assert_eq!(repeated.rejected_ids, ["a3"]);
    assert!(!repeated.passed);
    assert!(
        unknown
            .floors
            .iter()
            .all(|result| result.status == FloorStatus::Unavailable)
    );
    assert_eq!(unknown.rejected_ids, ["stranger"]);
    assert_eq!(unknown.missing, 1);
    assert!(unknown.to_markdown().contains("Rejected rows: stranger\n"));
}

#[test]
fn a_negative_citing_its_rows_expected_section_is_not_right() {
    let mut questions = passing();
    questions[84].expected = vec![right_section()];
    questions[84].ask = answered(vec![right_section()], 0);
    let score = score(&questions);

    assert_eq!(count(&score, Floor::Citation), (84, 85, FloorStatus::Pass));
    assert_eq!(count(&score, Floor::Refused), (15, 16, FloorStatus::Pass));
}

#[test]
fn a_score_without_asks_shows_its_ask_floors_not_run() {
    let mut questions = passing();
    for question in &mut questions {
        question.ask = ask_ending(AskOutcome::Failed);
    }
    let asked = score(&questions);
    let score = asked.clone().without_asks();

    let not_run: Vec<Floor> = score
        .floors
        .iter()
        .filter(|result| result.measure == Measure::NotRun { ran: false })
        .inspect(|result| assert_eq!(result.status, FloorStatus::Unavailable))
        .map(|result| result.floor)
        .collect();
    assert_eq!(
        not_run,
        [
            Floor::Refused,
            Floor::Citation,
            Floor::Answered,
            Floor::Literals,
            Floor::AskP95
        ]
    );
    assert_eq!(count(&score, Floor::Top10), (84, 84, FloorStatus::Pass));
    assert!(!score.passed && !score.asked);
    assert_eq!(score.failed_asks, 0);
    let markdown = score.to_markdown();
    assert!(markdown.contains("| Ask p95 | not run | not run | UNAVAILABLE |\n"));
    assert!(markdown.contains("Failed asks: not run\n"));
    assert!(markdown.contains("\nSupported answers: not run\nFalse refusals: not run\n"));
    assert!(
        asked
            .to_markdown()
            .contains("\nSupported answers: 0/84\nFalse refusals: 0/84\n")
    );
    let json = serde_json::to_value(&score).unwrap();
    assert_eq!(json["asked"], false);
    assert_eq!(
        json["floors"][7],
        serde_json::json!({"floor": "ask_p95", "status": "unavailable", "ran": false})
    );
}
