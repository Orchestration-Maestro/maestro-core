//! Model graph extraction is an explicit, mutually exclusive build mode.

use super::{
    graph_build::{COLLECTION, pilot},
    support::{Home, local},
};
use maestro_kernel::{
    facts::{Budget, BuildPlan},
    job::{self, NewJob},
    journal::Filter,
    scope::collection_path,
};
use maestro_knowledge::{
    graph::{
        build,
        rules::{Extractor as _, TableRule},
    },
    quality,
};
use serde_json::json;
use std::{fs, process::Command, time::UNIX_EPOCH};

#[test]
fn graph_build_help_documents_explicit_model_inputs() {
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args(["knowledge", "graph", "build", "--help"])
        .output()
        .expect("synthetic fixture is valid");
    assert!(output.status.success());
    let help = String::from_utf8(output.stdout).expect("synthetic fixture is valid");
    for flag in [
        "--rule",
        "--extractor-card",
        "--window-policy",
        "--token-budget",
    ] {
        assert!(help.contains(flag), "missing {flag} in {help}");
    }
    assert!(
        help.contains("estimate limit for one job"),
        "estimate scope is absent: {help}"
    );
    assert!(
        help.contains("failed-job rerun spends up to its own estimate"),
        "retry scope is absent: {help}"
    );
}

#[test]
fn model_extractor_inputs_and_estimate_are_frozen_in_the_job_created_event() {
    let home = Home::new();
    let rule = pilot(&home);
    let parsed = TableRule::parse(&fs::read_to_string(rule).unwrap()).unwrap();
    let database = home.database();
    let scopes = local(&database);
    let revisions = quality::eligible(&database, &scopes, COLLECTION).unwrap();
    let plan = BuildPlan {
        collection_id: COLLECTION.into(),
        provenance: parsed.provenance(),
        sources: revisions
            .iter()
            .map(|revision| revision.id.clone())
            .collect(),
        budget: Budget {
            max_claims: 100,
            max_rejections: 100,
        },
    };
    let inputs = build::inputs(
        &plan,
        Some(json!({"card_digest":"card","prompt_digest":"prompt",
            "window_policy_digest":"policy","estimated_tokens":1025})),
    );
    let scope = collection_path(COLLECTION).parse().unwrap();
    let job = database
        .submit_job(
            &NewJob {
                kind: "knowledge.graph.build",
                inputs: &inputs,
                scope: &scope,
                resource: Some("graph-build:synthetic-graph"),
            },
            UNIX_EPOCH,
        )
        .unwrap();
    let created = database
        .events(
            &scopes,
            &Filter {
                stream: &job::stream(job.id),
                after: 0,
                r#type: Some(job::CREATED),
            },
        )
        .unwrap();

    assert_eq!(
        created[0].data["inputs"]["extractor_inputs"]["estimated_tokens"],
        1025
    );
}

#[test]
fn graph_build_refuses_mixing_rule_and_model_inputs_before_any_model_call() {
    let output = Command::new(env!("CARGO_BIN_EXE_maestro"))
        .args([
            "knowledge",
            "graph",
            "build",
            "--collection",
            "synthetic",
            "--rule",
            "rule.json",
            "--extractor-card",
            &"a".repeat(64),
            "--window-policy",
            "policy.json",
            "--token-budget",
            "100",
        ])
        .output()
        .expect("synthetic fixture is valid");
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be used with"));
}
