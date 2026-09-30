mod support;

use super::super::job::{self as graph_job, Work};
use crate::{cli::output::Output, failure::Failure};
use maestro_kernel::{
    job::{self as kernel_job, JobState, LeaseTiming, NewJob},
    journal::Filter,
    scope::collection_path,
};
use maestro_knowledge::graph::build;
use serde_json::json;
use std::{
    fs,
    sync::atomic::Ordering,
    time::{Duration, UNIX_EPOCH},
};
use support::{extractor, fixture, frozen_inputs, plan};

#[test]
fn over_budget_plan_is_refused_before_fake_model_extract() {
    let fixture = fixture(&["Source text.\n"]);
    let (extractor, extracts, _, card_path) = extractor(&fixture, 10);
    let plan = plan(&fixture.revisions, &extractor);
    let result = graph_job::run(
        &fixture.kernel,
        Output::new(true),
        &Work {
            plan: &plan,
            revisions: &fixture.revisions,
            extractor: &extractor,
        },
    );
    assert!(matches!(result, Err(Failure::Refused(_))));
    assert_eq!(extracts.load(Ordering::Relaxed), 0);
    fs::remove_dir_all(card_path).unwrap();
}

#[test]
fn failed_job_rerun_gets_its_own_preflight_and_retry_budget() {
    let fixture = fixture(&["Source text.\n"]);
    let (extractor, extracts, _, card_path) = extractor(&fixture, 2048);
    let plan = plan(&fixture.revisions, &extractor);
    let inputs = frozen_inputs(
        &plan,
        &fixture.revisions,
        &extractor,
        &fixture.kernel.database,
    );
    let scope = collection_path("graph-test").parse().unwrap();
    let new = NewJob {
        kind: "knowledge.graph.build",
        inputs: &inputs,
        scope: &scope,
        resource: Some("graph-build:graph-test"),
    };
    let previous = fixture
        .kernel
        .database
        .submit_job(&new, UNIX_EPOCH)
        .unwrap();
    let mut lease = fixture
        .kernel
        .database
        .take_job(previous.id, "failed", UNIX_EPOCH, Duration::from_secs(1))
        .unwrap();
    fixture
        .kernel
        .database
        .begin_graph_build(&fixture.kernel.scopes, &lease, &plan)
        .unwrap();
    for attempt in 1..=2 {
        fixture
            .kernel
            .database
            .progress(
                &mut lease,
                UNIX_EPOCH,
                Duration::from_secs(1),
                &json!({"kind":"maestro.graph.build.source_started.v1",
                "revision":fixture.revisions[0].id, "attempt":attempt}),
            )
            .unwrap();
    }
    fixture
        .kernel
        .database
        .complete_job(
            &lease,
            JobState::Failed,
            &json!({"error":"synthetic crash"}),
        )
        .unwrap();

    let built = graph_job::run(
        &fixture.kernel,
        Output::new(true),
        &Work {
            plan: &plan,
            revisions: &fixture.revisions,
            extractor: &extractor,
        },
    )
    .unwrap();

    assert_ne!(built.job, previous.id);
    let created = fixture
        .kernel
        .database
        .events(
            &fixture.kernel.scopes,
            &Filter {
                stream: &kernel_job::stream(built.job),
                after: 0,
                r#type: Some(kernel_job::CREATED),
            },
        )
        .unwrap();
    assert_eq!(
        created[0].data["inputs"]["extractor_inputs"]["estimated_tokens"],
        1025
    );
    assert_eq!(extracts.load(Ordering::Relaxed), 1);
    fs::remove_dir_all(card_path).unwrap();
}

#[test]
fn second_source_restart_is_refused_after_another_source_used_the_restart() {
    let fixture = fixture(&["First source.\n", "Second source.\n"]);
    let (extractor, extracts, _, card_path) = extractor(&fixture, 4096);
    let plan = plan(&fixture.revisions, &extractor);
    let inputs = frozen_inputs(
        &plan,
        &fixture.revisions,
        &extractor,
        &fixture.kernel.database,
    );
    let scope = collection_path("graph-test").parse().unwrap();
    let new = NewJob {
        kind: "knowledge.graph.build",
        inputs: &inputs,
        scope: &scope,
        resource: Some("graph-build:graph-test"),
    };
    let job = fixture
        .kernel
        .database
        .submit_job(&new, UNIX_EPOCH)
        .unwrap();
    let mut lease = fixture
        .kernel
        .database
        .take_job(job.id, "crashed", UNIX_EPOCH, Duration::from_secs(1))
        .unwrap();
    fixture
        .kernel
        .database
        .begin_graph_build(&fixture.kernel.scopes, &lease, &plan)
        .unwrap();
    for (revision, attempt) in [
        (&fixture.revisions[0], 1),
        (&fixture.revisions[0], 2),
        (&fixture.revisions[1], 1),
    ] {
        fixture
            .kernel
            .database
            .progress(
                &mut lease,
                UNIX_EPOCH,
                Duration::from_secs(1),
                &json!({
                    "kind": "maestro.graph.build.source_started.v1",
                    "revision": revision.id,
                    "attempt": attempt
                }),
            )
            .unwrap();
    }
    let first = build::extract(
        &fixture.kernel.database,
        &extractor,
        &fixture.revisions[0],
        0,
    )
    .unwrap();
    fixture
        .kernel
        .database
        .record_graph_batch(
            &fixture.kernel.scopes,
            &mut lease,
            LeaseTiming {
                now: UNIX_EPOCH,
                term: Duration::from_secs(1),
            },
            &first,
        )
        .unwrap();
    extracts.store(0, Ordering::Relaxed);

    let result = graph_job::run(
        &fixture.kernel,
        Output::new(true),
        &Work {
            plan: &plan,
            revisions: &fixture.revisions,
            extractor: &extractor,
        },
    );

    assert!(matches!(result, Err(Failure::Refused(message)) if message.contains("retry limit")));
    assert_eq!(
        extracts.load(Ordering::Relaxed),
        0,
        "refusal precedes extraction"
    );
    fs::remove_dir_all(card_path).unwrap();
}

#[test]
fn resumed_build_preflights_all_sources_without_reextracting_committed_batch() {
    let fixture = fixture(&["First source.\n", "Second source.\n"]);
    let (extractor, extracts, tokenizes, card_path) = extractor(&fixture, 4096);
    let plan = plan(&fixture.revisions, &extractor);
    let inputs = frozen_inputs(
        &plan,
        &fixture.revisions,
        &extractor,
        &fixture.kernel.database,
    );
    let scope = collection_path("graph-test").parse().unwrap();
    let new = NewJob {
        kind: "knowledge.graph.build",
        inputs: &inputs,
        scope: &scope,
        resource: Some("graph-build:graph-test"),
    };
    let job = fixture
        .kernel
        .database
        .submit_job(&new, UNIX_EPOCH)
        .unwrap();
    let mut lease = fixture
        .kernel
        .database
        .take_job(job.id, "crashed", UNIX_EPOCH, Duration::from_secs(1))
        .unwrap();
    fixture
        .kernel
        .database
        .begin_graph_build(&fixture.kernel.scopes, &lease, &plan)
        .unwrap();
    let first = build::extract(
        &fixture.kernel.database,
        &extractor,
        &fixture.revisions[0],
        0,
    )
    .unwrap();
    fixture
        .kernel
        .database
        .record_graph_batch(
            &fixture.kernel.scopes,
            &mut lease,
            LeaseTiming {
                now: UNIX_EPOCH,
                term: Duration::from_secs(1),
            },
            &first,
        )
        .unwrap();
    extracts.store(0, Ordering::Relaxed);
    tokenizes.store(0, Ordering::Relaxed);

    graph_job::run(
        &fixture.kernel,
        Output::new(true),
        &Work {
            plan: &plan,
            revisions: &fixture.revisions,
            extractor: &extractor,
        },
    )
    .unwrap();

    assert_eq!(tokenizes.load(Ordering::Relaxed), 2, "whole plan preflight");
    assert_eq!(
        extracts.load(Ordering::Relaxed),
        1,
        "only the uncommitted source"
    );
    fs::remove_dir_all(card_path).unwrap();
}
