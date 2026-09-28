//! The ladder's run: warm-ups unscored, the rung's configuration in every
//! search and ask, drift INVALID, and refusals before any search.

use super::{
    super::runner::{Verdict, run_ladder},
    support::{FakeEngine, rung, suite},
};
use crate::failure::Failure;
use maestro_knowledge::eval::{Floor, FloorStatus, Measure};

#[test]
fn warm_ups_run_first_and_are_not_scored() {
    let mut engine = FakeEngine::default();
    let suite = suite(3, 1);
    let runs = run_ladder(&mut engine, &suite, 2, &[rung("r0")], |_| Ok(())).unwrap();

    let searched = engine.questions("search");
    assert_eq!(searched.len(), 6);
    assert_eq!(searched[..2], searched[2..4]);
    assert_eq!(engine.questions("ask").len(), 6);
    let run = &runs[0];
    assert_eq!(run.warm_ups, 2);
    let ids: Vec<&str> = run.rows.iter().map(|row| row.id.as_str()).collect();
    assert_eq!(ids, ["a0", "a1", "a2", "u0"]);
    assert_eq!(run.score.missing, 0);
    assert!(run.score.rejected_ids.is_empty());
}

#[test]
fn each_search_and_ask_runs_under_its_rungs_configuration() {
    let mut engine = FakeEngine::default();
    let first = rung("r0");
    let mut second = rung("r1");
    second.configuration.rerank = None;
    second.configuration.weights.dense = 0.5;
    run_ladder(
        &mut engine,
        &suite(2, 1),
        0,
        &[first.clone(), second.clone()],
        |_| Ok(()),
    )
    .unwrap();

    let calls = engine.calls.borrow();
    assert_eq!(calls.len(), 12);
    for call in calls.iter() {
        let expected = if call.rung == "r0" { &first } else { &second };
        assert_eq!(call.configuration, expected.configuration.search());
    }
    assert!(
        calls
            .iter()
            .any(|call| call.operation == "ask" && call.rung == "r1")
    );
    assert!(
        calls[..6]
            .iter()
            .all(|call| call.configuration.rerank_enabled)
    );
    assert!(
        calls[6..]
            .iter()
            .all(|call| !call.configuration.rerank_enabled)
    );
}

#[test]
fn a_rung_whose_generation_changes_is_invalid() {
    let mut engine = FakeEngine {
        drift_after: Some(2),
        ..FakeEngine::default()
    };
    let runs = run_ladder(&mut engine, &suite(2, 1), 0, &[rung("r0")], |_| Ok(())).unwrap();

    assert_ne!(Some(&runs[0].start), runs[0].end.as_ref());
    assert_eq!(runs[0].verdict(), Verdict::Invalid);
    assert!(runs[0].score.passed);
}

#[test]
fn a_rung_that_starts_on_another_generation_than_the_first_is_invalid() {
    let mut engine = FakeEngine {
        drift_at_start_of: Some("r1".to_owned()),
        ..FakeEngine::default()
    };
    let rungs = [rung("r0"), rung("r1"), rung("r2")];
    let runs = run_ladder(&mut engine, &suite(2, 1), 0, &rungs, |_| Ok(())).unwrap();

    assert_eq!(runs[0].verdict(), Verdict::Pass);
    for run in &runs[1..] {
        assert_eq!(Some(&run.start), run.end.as_ref());
        assert_eq!(run.ladder, runs[0].start);
        assert_eq!(run.verdict(), Verdict::Invalid);
    }
}

#[test]
fn rungs_may_differ_in_their_rerankers_alone() {
    let mut engine = FakeEngine::default();
    let mut unranked = rung("r1");
    unranked.configuration.rerank = None;
    let runs = run_ladder(
        &mut engine,
        &suite(2, 1),
        0,
        &[rung("r0"), unranked],
        |_| Ok(()),
    )
    .unwrap();

    assert_ne!(runs[1].start, runs[0].start);
    assert_eq!(runs[1].verdict(), Verdict::Pass);
}

#[test]
fn a_dense_rung_without_an_embedder_card_is_refused_before_any_search() {
    let mut engine = FakeEngine {
        no_embedder: true,
        ..FakeEngine::default()
    };
    let mut lexical = rung("r0");
    lexical.configuration.routes.dense = false;
    let result = run_ladder(&mut engine, &suite(2, 1), 0, &[lexical, rung("r1")], |_| {
        Ok(())
    });

    assert!(matches!(
        result,
        Err(Failure::Refused(reason)) if reason.contains("`r1` runs dense retrieval")
    ));
    assert!(engine.calls.borrow().is_empty());
}

#[test]
fn an_asking_rung_without_an_answerer_card_is_refused_before_any_search() {
    let mut engine = FakeEngine {
        no_answerer: true,
        ..FakeEngine::default()
    };
    let mut retrieval = rung("r0");
    retrieval.ask = None;
    let result = run_ladder(
        &mut engine,
        &suite(2, 1),
        0,
        &[retrieval.clone(), rung("r1")],
        |_| Ok(()),
    );

    assert!(matches!(
        result,
        Err(Failure::Refused(reason)) if reason.contains("`r1` asks")
    ));
    assert!(engine.calls.borrow().is_empty());
    assert!(run_ladder(&mut engine, &suite(2, 1), 0, &[retrieval], |_| Ok(())).is_ok());
}

#[test]
fn a_rung_that_does_not_ask_shows_its_ask_floors_not_run() {
    let mut engine = FakeEngine::default();
    let mut retrieval = rung("r0");
    retrieval.ask = None;
    let runs = run_ladder(&mut engine, &suite(2, 1), 1, &[retrieval], |_| Ok(())).unwrap();

    assert!(engine.questions("ask").is_empty());
    let statuses: Vec<(Floor, FloorStatus, bool)> = runs[0]
        .score
        .floors
        .iter()
        .map(|result| {
            let not_run = result.measure == Measure::NotRun { ran: false };
            (result.floor, result.status, not_run)
        })
        .collect();
    assert_eq!(
        statuses,
        [
            (Floor::Top10, FloorStatus::Pass, false),
            (Floor::Top1, FloorStatus::Pass, false),
            (Floor::Refused, FloorStatus::Unavailable, true),
            (Floor::Citation, FloorStatus::Unavailable, true),
            (Floor::Answered, FloorStatus::Unavailable, true),
            (Floor::Literals, FloorStatus::Unavailable, true),
            (Floor::SearchP95, FloorStatus::Pass, false),
            (Floor::AskP95, FloorStatus::Unavailable, true),
        ]
    );
    assert_eq!(runs[0].score.failed_asks, 0);
    assert!(!runs[0].score.asked);
    assert_eq!(runs[0].verdict(), Verdict::Fail);
}

#[test]
fn a_rung_that_cannot_run_is_refused_before_any_search() {
    let mut engine = FakeEngine {
        refused_rung: Some("r1".to_owned()),
        ..FakeEngine::default()
    };
    let result = run_ladder(
        &mut engine,
        &suite(2, 1),
        0,
        &[rung("r0"), rung("r1")],
        |_| Ok(()),
    );

    assert!(matches!(result, Err(Failure::Refused(_))));
    assert!(engine.calls.borrow().is_empty());
}

#[test]
fn more_warm_ups_than_questions_are_refused_and_as_many_are_not() {
    let mut engine = FakeEngine::default();
    let result = run_ladder(&mut engine, &suite(2, 1), 4, &[rung("r0")], |_| Ok(()));

    assert!(matches!(result, Err(Failure::Refused(reason)) if reason.contains("warm-ups")));
    assert!(engine.calls.borrow().is_empty());
    let every = run_ladder(&mut engine, &suite(2, 1), 3, &[rung("r0")], |_| Ok(()));
    assert_eq!(every.unwrap()[0].warm_ups, 3);
}

#[test]
fn a_recording_failure_stops_the_ladder() {
    let mut engine = FakeEngine::default();
    let mut recorded = Vec::new();
    let result = run_ladder(
        &mut engine,
        &suite(2, 1),
        0,
        &[rung("r0"), rung("r1")],
        |run| {
            recorded.push(run.rung.name.clone());
            Err(Failure::failed("disk full"))
        },
    );

    assert!(matches!(result, Err(Failure::Failed(_))));
    assert_eq!(recorded, ["r0"]);
    assert!(engine.calls.borrow().iter().all(|call| call.rung == "r0"));
}

#[test]
fn a_rung_whose_end_cannot_be_read_is_recorded_invalid_then_stops_the_ladder() {
    let mut engine = FakeEngine {
        unreadable_after: Some(1),
        ..FakeEngine::default()
    };
    let mut recorded = Vec::new();

    let stopped = run_ladder(
        &mut engine,
        &suite(2, 1),
        0,
        &[rung("r0"), rung("r1")],
        |run| {
            recorded.push(run.clone());
            Ok(())
        },
    );

    assert!(matches!(stopped, Err(Failure::Failed(_))), "{stopped:?}");
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].end, None);
    assert_eq!(recorded[0].rows.len(), 3);
    assert_eq!(recorded[0].verdict(), Verdict::Invalid);
}

#[test]
fn a_rung_whose_expected_sections_miss_a_question_is_refused_before_any_search() {
    let mut engine = FakeEngine {
        expectations_missing: 1,
        ..FakeEngine::default()
    };

    let refused = run_ladder(&mut engine, &suite(2, 1), 0, &[rung("r0")], |_| Ok(()));

    assert!(
        matches!(
            &refused,
            Err(Failure::Refused(reason)) if reason
                == "the rung `r0` resolved expected sections for 2 of the suite's 3 questions"
        ),
        "{refused:?}"
    );
    assert!(engine.calls.borrow().is_empty());
}
