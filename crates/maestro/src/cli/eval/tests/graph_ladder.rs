//! Existing ladder rows earn graph credit only from successful delivered anchors.

use super::{
    super::{graph_ladder::score_run, runner::run_ladder},
    support::{FakeEngine, rung, suite},
};
use maestro_kernel::artifact::Digest;
use maestro_knowledge::eval::{
    AskOutcome, CheckedItem, CheckedLabels, LabelSummary, Located, QuestionKind, SearchOutcome,
};
use std::{collections::BTreeMap, convert::Infallible};

#[test]
fn graph_eval_ladder_scores_delivered_proofs_and_excludes_failed_attempts() {
    let suite = suite(2, 0);
    let mut engine = FakeEngine::default();
    let mut runs = run_ladder(&mut engine, &suite, 0, &[rung("passage-only")], |_| Ok(())).unwrap();
    let run = &mut runs[0];
    let anchor = Located {
        revision_id: "revision-1".to_owned(),
        span: [0, 5],
    };
    let labels = CheckedLabels {
        items: suite
            .questions
            .iter()
            .map(|question| CheckedItem {
                id: question.id.clone(),
                family: question.id.clone(),
                kind: QuestionKind::Relationship,
                proofs: vec![vec![anchor.clone()]],
            })
            .collect(),
        summary: LabelSummary {
            items: 2,
            answerable: 2,
            unanswerable: 0,
            families: 2,
            links: 2,
            anchors: 2,
            unreviewed: 0,
            digest: Digest::of(b"labels"),
            kinds: BTreeMap::new(),
            languages: BTreeMap::new(),
        },
    };
    for diagnostic in &mut run.diagnostics {
        diagnostic.delivered = vec![anchor.clone()];
    }
    assert_eq!(
        (
            score_run(run, &labels).unwrap().final_wire.complete,
            score_run(run, &labels).unwrap().final_wire.of
        ),
        (2, 2)
    );
    let score = score_run(run, &labels).unwrap();
    assert_eq!(
        (score.stages[3].complete.count, score.stages[3].complete.of),
        (2, 2)
    );
    assert_eq!(
        (
            score.stages[0].complete.of,
            score.stages[0].complete.unobserved
        ),
        (0, 2)
    );
    assert_eq!(score.conclusions.unobserved_items, 2);
    let mut counts = super::super::graph_output::Counts {
        links: 8,
        anchors: 8,
        complete: 8,
        answerable: 8,
        ..super::super::graph_output::Counts::default()
    };
    for _ in 0..2 {
        super::super::graph_ladder::record_counts(&mut counts, run, &labels, &score);
    }
    assert_eq!(
        (
            counts.links,
            counts.anchors,
            counts.complete,
            counts.answerable
        ),
        (12, 12, 12, 12)
    );
    assert_eq!(counts.failed, 0);
    run.diagnostics[0].delivered.clear();
    assert_eq!(score_run(run, &labels).unwrap().final_wire.complete, 1);
    run.rows[1].search.outcome = SearchOutcome::TimedOut;
    assert_eq!(score_run(run, &labels).unwrap().final_wire.complete, 0);
    assert_eq!(run.diagnostics[1].delivered, [anchor]);
}

#[test]
fn graph_eval_private_ladder_counts_unreviewed_unanswerable_and_failed_asks() {
    use super::super::{graph_ladder::record_counts, graph_output::Counts};
    use maestro_knowledge::eval::{Stage, check_labels};
    use serde_json::json;
    let suite = suite(0, 1);
    let text = json!({"schema":"maestro-graph-labels/1","id":"u0","family":"f1",
        "kind":"unanswerable","language":"fr","proofs":[],
        "unanswerable_reason":"no supporting source"})
    .to_string();
    let labels = check_labels(
        &suite,
        &text,
        &Digest::of(text.as_bytes()),
        Stage::Draft,
        |_, _| Ok::<_, Infallible>(None),
    )
    .unwrap();
    let mut engine = FakeEngine {
        ask_outcome: Some(AskOutcome::Failed),
        ..FakeEngine::default()
    };
    let mut counts = Counts::default();
    run_ladder(
        &mut engine,
        &suite,
        0,
        &[rung("first"), rung("second")],
        |run| {
            let score = score_run(run, &labels)?;
            record_counts(&mut counts, run, &labels, &score);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(counts.items, 2);
    assert_eq!(counts.unreviewed, 2);
    assert_eq!(counts.unanswerable, 2);
    assert_eq!(counts.answerable, 0);
    assert_eq!(counts.complete, 0);
    assert_eq!(counts.failed, 2);
}
