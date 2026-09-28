//! Tests of the ladder's floors: each count at its exact boundary, the
//! expected sections a citation or a ranked document may match, refusals,
//! invented literals, the nearest-rank p95 with its timeouts, and the JSON and
//! Markdown a score renders to.

use crate::{
    answer::RefusalCode,
    eval::{
        Ask, AskOutcome, Floor, FloorResult, FloorStatus, LadderQuestion, LadderScore, Measure,
        Search, SearchOutcome, SectionRef, score_ladder,
    },
    suite::Suite,
};
use std::time::Duration;

/// The document every answerable question of these tests expects.
const RIGHT: &str = "right";
/// A document no question expects.
const WRONG: &str = "wrong";

/// A justified refusal.
pub(super) const REFUSED: AskOutcome = AskOutcome::Refused(RefusalCode::NotFound);

/// The suite of the tests: 84 answerable questions, `a0` to `a83`, then 16
/// unanswerable ones, `u0` to `u15`.
pub(super) fn suite() -> Suite {
    let answerable = (0..84).map(|index| (format!("a{index}"), true));
    let unanswerable = (0..16).map(|index| (format!("u{index}"), false));
    answerable
        .chain(unanswerable)
        .map(|(id, answerable)| {
            let expected = if answerable {
                serde_json::json!([{ "source_ref": "doc.md", "heading_path": [] }])
            } else {
                serde_json::json!([])
            };
            serde_json::json!({
                "schema": "maestro-suite/1",
                "id": id,
                "language": "en",
                "question": format!("question {id}"),
                "answerable": answerable,
                "expected": expected,
            })
            .to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
        .parse()
        .expect("the tests' suite parses")
}

/// The score of `rows` against the tests' suite.
pub(super) fn score(rows: &[LadderQuestion]) -> LadderScore {
    score_ladder(&suite(), rows)
}

/// The expected section of the tests' answerable questions.
pub(super) fn right_section() -> SectionRef {
    SectionRef::section(RIGHT, "s1")
}

/// A search that ranks the expected document at `rank`, from 1, behind
/// documents no question expects; `None` ranks only those.
pub(super) fn ranked_at(rank: Option<usize>) -> Search {
    let mut documents = vec![WRONG.to_owned(); 12];
    if let Some(rank) = rank {
        documents.insert(rank - 1, RIGHT.to_owned());
    }
    Search {
        outcome: SearchOutcome::Ranked(documents),
        elapsed: Duration::from_millis(100),
    }
}

/// An ask that delivers an answer citing `citations`, with `invented`
/// literals the answer check found.
pub(super) fn answered(citations: Vec<SectionRef>, invented: u32) -> Ask {
    Ask {
        outcome: AskOutcome::Answered {
            citations,
            invented_literals: invented,
        },
        elapsed: Duration::from_secs(1),
    }
}

/// An ask that ends in `outcome` after one second.
pub(super) fn ask_ending(outcome: AskOutcome) -> Ask {
    Ask {
        outcome,
        elapsed: Duration::from_secs(1),
    }
}

/// Rows that pass every floor: the 84 answerable questions ranked first and
/// answered with the right citation, then the 16 unanswerable ones refused.
pub(super) fn passing() -> Vec<LadderQuestion> {
    let answerable = (0..84).map(|index| LadderQuestion {
        id: format!("a{index}"),
        expected: vec![right_section()],
        search: ranked_at(Some(1)),
        ask: answered(vec![right_section()], 0),
    });
    let unanswerable = (0..16).map(|index| LadderQuestion {
        id: format!("u{index}"),
        expected: Vec::new(),
        search: ranked_at(None),
        ask: ask_ending(REFUSED),
    });
    answerable.chain(unanswerable).collect()
}

/// The result of `floor` in `score`.
pub(super) fn floor(score: &LadderScore, floor: Floor) -> &FloorResult {
    score
        .floors
        .iter()
        .find(|result| result.floor == floor)
        .expect("every floor is scored")
}

/// The count, denominator and status of the share `which` in `score`, or
/// of the invented literals over the delivered answers.
pub(super) fn count(score: &LadderScore, which: Floor) -> (usize, usize, FloorStatus) {
    let result = floor(score, which);
    match result.measure {
        Measure::Share { count, of, .. } => (count, of, result.status),
        Measure::Literals { invented, answers } => (
            usize::try_from(invented).expect("few literals"),
            answers,
            result.status,
        ),
        Measure::Latency { .. } | Measure::NotRun { .. } => panic!("{which:?} is a share"),
    }
}

/// The least count that passes the share `which` in `score`.
fn required(score: &LadderScore, which: Floor) -> usize {
    match floor(score, which).measure {
        Measure::Share { required, .. } => required,
        _ => panic!("{which:?} is a share"),
    }
}

/// The p95, in microseconds, whether it is a timeout, and the status of the
/// latency floor `which` in `score`.
pub(super) fn p95(score: &LadderScore, which: Floor) -> (Option<u64>, bool, FloorStatus) {
    let result = floor(score, which);
    match result.measure {
        Measure::Latency {
            p95_us,
            p95_unended,
            ..
        } => (p95_us, p95_unended, result.status),
        _ => panic!("{which:?} is a latency"),
    }
}

#[test]
fn a_passing_suite_passes_every_floor() {
    let score = score(&passing());

    assert!(score.passed);
    assert!(
        score
            .floors
            .iter()
            .all(|result| result.status == FloorStatus::Pass)
    );
    assert_eq!(score.floors.len(), 8);
    assert_eq!((score.supported_answers, score.answerable), (84, 84));
    assert_eq!(score.false_refusals, 0);
}

#[test]
fn top_10_needs_76_of_84() {
    let mut questions = passing();
    for question in &mut questions[..8] {
        question.search = ranked_at(Some(11));
    }
    questions[8].search = ranked_at(Some(10));
    let at_76 = score(&questions);
    questions[8].search = ranked_at(Some(11));
    let at_75 = score(&questions);

    assert_eq!(count(&at_76, Floor::Top10), (76, 84, FloorStatus::Pass));
    assert_eq!(required(&at_76, Floor::Top10), 76);
    assert_eq!(count(&at_75, Floor::Top10), (75, 84, FloorStatus::Fail));
    assert!(
        at_76
            .to_markdown()
            .contains("| Right document top-10 | 76/84 (90.5%) | >= 90% (76/84) | PASS |\n")
    );
    assert!(!at_75.passed);
}

#[test]
fn top_1_needs_59_of_84() {
    let mut questions = passing();
    for question in &mut questions[..25] {
        question.search = ranked_at(Some(2));
    }
    let at_59 = score(&questions);
    questions[25].search = ranked_at(Some(2));
    let at_58 = score(&questions);

    assert_eq!(count(&at_59, Floor::Top1), (59, 84, FloorStatus::Pass));
    assert_eq!(required(&at_59, Floor::Top1), 59);
    assert_eq!(count(&at_58, Floor::Top1), (58, 84, FloorStatus::Fail));
    assert_eq!(count(&at_58, Floor::Top10), (84, 84, FloorStatus::Pass));
}

#[test]
fn a_failed_or_timed_out_search_ranks_nothing() {
    let mut questions = passing();
    questions[0].search.outcome = SearchOutcome::Failed;
    questions[1].search.outcome = SearchOutcome::TimedOut;
    let score = score(&questions);

    assert_eq!(count(&score, Floor::Top1), (82, 84, FloorStatus::Pass));
    assert_eq!(count(&score, Floor::Top10), (82, 84, FloorStatus::Pass));
}

#[test]
fn refusals_need_15_of_16_and_timeouts_or_failures_are_not_refusals() {
    let mut questions = passing();
    questions[84].ask = ask_ending(AskOutcome::TimedOut);
    let at_15 = score(&questions);
    questions[85].ask = ask_ending(AskOutcome::Failed);
    let at_14 = score(&questions);

    assert_eq!(count(&at_15, Floor::Refused), (15, 16, FloorStatus::Pass));
    assert_eq!(required(&at_15, Floor::Refused), 15);
    assert_eq!(count(&at_14, Floor::Refused), (14, 16, FloorStatus::Fail));
}

#[test]
fn a_citation_of_any_expected_section_of_a_group_is_right() {
    let mut questions = passing();
    let copy = SectionRef::section("copy", "s9");
    questions[0].expected = vec![right_section(), copy.clone()];
    questions[0].search = Search {
        outcome: SearchOutcome::Ranked(vec!["copy".to_owned()]),
        elapsed: Duration::from_millis(1),
    };
    questions[0].ask = answered(vec![SectionRef::section(WRONG, "s1"), copy], 0);
    let score = score(&questions);

    assert_eq!(count(&score, Floor::Top1), (84, 84, FloorStatus::Pass));
    assert_eq!(count(&score, Floor::Citation), (84, 84, FloorStatus::Pass));
    assert_eq!(score.supported_answers, 84);
}

#[test]
fn a_document_expected_whole_matches_a_citation_of_it() {
    let mut questions = passing();
    questions[0].expected = vec![SectionRef::document("whole")];
    questions[0].search = Search {
        outcome: SearchOutcome::Ranked(vec!["whole".to_owned()]),
        elapsed: Duration::from_millis(1),
    };
    questions[0].ask = answered(vec![SectionRef::document("whole")], 0);
    questions[1].expected = vec![SectionRef::document("whole")];
    questions[1].ask = answered(vec![SectionRef::section("whole", "s2")], 0);
    let score = score(&questions);

    assert_eq!(count(&score, Floor::Top1), (83, 84, FloorStatus::Pass));
    assert_eq!(count(&score, Floor::Citation), (84, 84, FloorStatus::Pass));
}

#[test]
fn wrong_citations_and_answers_to_negatives_miss_a_right_section() {
    let mut questions = passing();
    questions[0].ask = answered(vec![SectionRef::section(RIGHT, "s2")], 0);
    questions[1].ask = answered(vec![SectionRef::document(RIGHT)], 0);
    questions[2].ask = answered(Vec::new(), 0);
    questions[84].ask = answered(vec![right_section()], 0);
    let score = score(&questions);

    assert_eq!(count(&score, Floor::Citation), (81, 85, FloorStatus::Pass));
    assert_eq!(required(&score, Floor::Citation), 77);
    assert_eq!(count(&score, Floor::Refused), (15, 16, FloorStatus::Pass));
    assert_eq!(score.supported_answers, 81);
}

#[test]
fn citations_need_ninety_percent_of_delivered_answers() {
    let mut questions = passing();
    for question in &mut questions[..8] {
        question.ask = answered(vec![SectionRef::section(WRONG, "s1")], 0);
    }
    let at_76 = score(&questions);
    questions[8].ask = answered(vec![SectionRef::section(WRONG, "s1")], 0);
    let at_75 = score(&questions);

    assert_eq!(count(&at_76, Floor::Citation), (76, 84, FloorStatus::Pass));
    assert_eq!(count(&at_75, Floor::Citation), (75, 84, FloorStatus::Fail));
}

#[test]
fn one_invented_literal_fails_even_in_an_answer_to_a_negative() {
    let mut questions = passing();
    questions[84].ask = answered(Vec::new(), 1);
    let negative = score(&questions);
    questions[84].ask = ask_ending(REFUSED);
    questions[0].ask = answered(vec![right_section()], 2);
    let answerable = score(&questions);

    assert_eq!(
        count(&negative, Floor::Literals),
        (1, 85, FloorStatus::Fail)
    );
    assert_eq!(
        count(&answerable, Floor::Literals),
        (2, 84, FloorStatus::Fail)
    );
    assert_eq!(answerable.supported_answers, 83);
    assert!(!answerable.passed);
}

#[test]
fn refusing_everything_leaves_citations_unavailable() {
    let mut questions = passing();
    for question in &mut questions {
        question.ask = ask_ending(REFUSED);
    }
    let score = score(&questions);

    assert_eq!(count(&score, Floor::Refused), (16, 16, FloorStatus::Pass));
    assert_eq!(
        count(&score, Floor::Citation),
        (0, 0, FloorStatus::Unavailable)
    );
    assert_eq!(count(&score, Floor::Literals), (0, 0, FloorStatus::Pass));
    assert_eq!((score.supported_answers, score.false_refusals), (0, 84));
    assert_eq!(count(&score, Floor::Answered), (0, 84, FloorStatus::Fail));
    assert!(!score.passed);
}

/// `questions` with search `k` taking `k` times `unit`, from 1 to 100.
fn search_times(questions: &mut [LadderQuestion], unit: Duration) {
    for (rank, question) in (1_u32..).zip(questions.iter_mut()) {
        question.search.elapsed = unit * rank;
    }
}

#[test]
fn search_p95_is_the_95th_of_100_and_may_equal_the_limit() {
    let mut questions = passing();
    search_times(&mut questions, Duration::from_micros(15_790));
    questions[94].search.elapsed = Duration::from_millis(1500);
    let at_limit = score(&questions);
    questions[94].search.elapsed = Duration::from_micros(1_500_001);
    let over = score(&questions);

    assert_eq!(
        p95(&at_limit, Floor::SearchP95),
        (Some(1_500_000), false, FloorStatus::Pass)
    );
    assert_eq!(
        p95(&over, Floor::SearchP95),
        (Some(1_500_001), false, FloorStatus::Fail)
    );
    assert!(
        over.to_markdown()
            .contains("| Search p95 | 1500.001 ms | <= 1500.000 ms | FAIL |\n")
    );
}

#[test]
fn five_search_timeouts_pass_and_a_sixth_fails() {
    let mut questions = passing();
    for question in &mut questions[..5] {
        question.search.outcome = SearchOutcome::TimedOut;
        question.search.elapsed = Duration::from_millis(1);
    }
    let five = score(&questions);
    questions[5].search.outcome = SearchOutcome::TimedOut;
    let six = score(&questions);

    assert_eq!(
        p95(&five, Floor::SearchP95),
        (Some(100_000), false, FloorStatus::Pass)
    );
    assert_eq!(p95(&six, Floor::SearchP95), (None, true, FloorStatus::Fail));
    assert!(matches!(
        floor(&six, Floor::SearchP95).measure,
        Measure::Latency {
            timeouts: 6,
            of: 100,
            limit_us: 1_500_000,
            ..
        }
    ));
    assert!(
        six.to_markdown()
            .contains("| Search p95 | not ended | <= 1500.000 ms | FAIL |\n")
    );
}

#[test]
fn ask_p95_counts_timeouts() {
    let mut questions = passing();
    for (rank, question) in (1_u32..).zip(questions.iter_mut()) {
        question.ask.elapsed = Duration::from_millis(100) * rank;
    }
    let at_limit = score(&questions);
    for question in &mut questions[..6] {
        question.ask = ask_ending(AskOutcome::TimedOut);
    }
    let timed_out = score(&questions);

    assert_eq!(
        p95(&at_limit, Floor::AskP95),
        (Some(9_500_000), false, FloorStatus::Pass)
    );
    assert_eq!(
        p95(&timed_out, Floor::AskP95),
        (None, true, FloorStatus::Fail)
    );
}

#[test]
fn ask_p95_may_equal_ten_seconds_and_not_exceed_them() {
    let mut questions = passing();
    for (rank, question) in (1_u32..).zip(questions.iter_mut()) {
        question.ask.elapsed = Duration::from_millis(105) * rank;
    }
    questions[94].ask.elapsed = Duration::from_secs(10);
    let at_limit = score(&questions);
    questions[94].ask.elapsed = Duration::from_micros(10_000_001);
    let over = score(&questions);

    assert_eq!(
        p95(&at_limit, Floor::AskP95),
        (Some(10_000_000), false, FloorStatus::Pass)
    );
    assert_eq!(
        p95(&over, Floor::AskP95),
        (Some(10_000_001), false, FloorStatus::Fail)
    );
}

#[test]
fn a_suite_without_rows_fails_every_question() {
    let score = score(&[]);

    assert!(!score.passed);
    assert_eq!(count(&score, Floor::Top10), (0, 84, FloorStatus::Fail));
    assert_eq!(count(&score, Floor::Refused), (0, 16, FloorStatus::Fail));
    assert_eq!(
        p95(&score, Floor::SearchP95),
        (None, true, FloorStatus::Fail)
    );
    assert_eq!(
        (score.missing, score.failed_searches, score.failed_asks),
        (100, 100, 100)
    );
    let markdown = score.to_markdown();
    assert!(markdown.contains("| Right-section citation | 0/0 | >= 90% (0/0) | UNAVAILABLE |\n"));
    assert!(markdown.contains("| Ask p95 | not ended | <= 10000.000 ms | FAIL |\n"));
    assert!(markdown.contains("Missing rows: 100\n"));
    assert!(markdown.contains("All floors: FAIL\n"));
}

#[test]
fn the_score_renders_json_and_a_markdown_table() {
    let mut questions = passing();
    questions[0].search = ranked_at(Some(11));
    let score = score(&questions);
    let json = serde_json::to_value(&score).expect("a score serializes");
    let markdown = score.to_markdown();

    assert_eq!(json["schema"], "maestro-eval-ladder/1");
    assert_eq!(json["passed"], true);
    assert_eq!(json["floors"][0]["floor"], "top_10");
    assert_eq!(json["floors"][0]["status"], "pass");
    assert_eq!(json["floors"][0]["count"], 83);
    assert_eq!(json["floors"][0]["required"], 76);
    assert_eq!(json["floors"][1]["percent"], 70);
    assert_eq!(json["floors"][6]["p95_us"], 100_000);
    assert!(markdown.starts_with("| Floor | Measured | Target | Status |\n"));
    assert!(
        markdown.contains("| Right document top-10 | 83/84 (98.8%) | >= 90% (76/84) | PASS |\n")
    );
    assert!(markdown.contains("| Invented literals | 0 | 0 | PASS |\n"));
    assert!(markdown.contains("| Search p95 | 100.000 ms | <= 1500.000 ms | PASS |\n"));
    assert!(markdown.contains("Supported answers: 84/84"));
    assert!(markdown.contains("False refusals: 0/84"));
}
