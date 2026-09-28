//! Durable CLI resume and separate attachment, using real authority and rule artifacts.

use super::{
    graph_build::{COLLECTION, build, envelope, exited, pilot, text, write},
    support::{Home, local},
};
use maestro_kernel::{
    facts::{Budget, BuildPlan},
    generation::NewGeneration,
    job::{JobState, LeaseTiming, NewJob},
    scope::collection_path,
};
use maestro_knowledge::{
    graph::{
        build as runner,
        rules::{Extractor as _, TableRule},
    },
    quality,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, UNIX_EPOCH},
};

/// A building generation, deliberately not published by this task.
fn generation(home: &Home) -> i64 {
    let database = home.database();
    let connection = Connection::open(home.data().join("kernel.sqlite3")).unwrap();
    connection
        .execute(
            "INSERT INTO chunk_sets (id, collection_id, chunk_profile, counter_contract_id, state)
         VALUES ('graph-test-set', ?1, 'structural/1', 'native', 'building')",
            [COLLECTION],
        )
        .unwrap();
    database
        .create_generation(&NewGeneration {
            collection_id: COLLECTION.into(),
            chunk_set_id: "graph-test-set".into(),
            embedding_profile: "embed:test".into(),
            sparse_profile: "bm25-en-fr/1".into(),
        })
        .unwrap()
        .id
}

#[test]
fn graph_resume_committed_batch_survives_takeover_without_source_io() {
    for committed in [false, true] {
        let home = Home::new();
        let rule_path = pilot(&home);
        let rule = TableRule::parse(&fs::read_to_string(&rule_path).unwrap()).unwrap();
        let database = home.database();
        let scopes = local(&database);
        let revisions = quality::eligible(&database, &scopes, COLLECTION).unwrap();
        let plan = BuildPlan {
            collection_id: COLLECTION.into(),
            provenance: rule.provenance(),
            sources: revisions
                .iter()
                .map(|revision| revision.id.clone())
                .collect(),
            budget: Budget {
                max_claims: 1_000_000,
                max_rejections: 1_000_000,
            },
        };
        let inputs = runner::inputs(&plan);
        let scope = collection_path(COLLECTION).parse().unwrap();
        let resource = format!("graph-build:{COLLECTION}");
        let new = NewJob {
            kind: "knowledge.graph.build",
            inputs: &inputs,
            scope: &scope,
            resource: Some(&resource),
        };
        let job = database.submit_job(&new, UNIX_EPOCH).unwrap();
        let mut lease = database
            .take_job(job.id, "interrupted", UNIX_EPOCH, Duration::from_secs(1))
            .unwrap();
        database.begin_graph_build(&scopes, &lease, &plan).unwrap();
        if committed {
            let batch = runner::extract(&database, &rule, &revisions[0], 0).unwrap();
            database
                .record_graph_batch(
                    &scopes,
                    &mut lease,
                    LeaseTiming {
                        now: UNIX_EPOCH,
                        term: Duration::from_secs(1),
                    },
                    &batch,
                )
                .unwrap();
            fs::remove_file(stored(&home, revisions[0].original_digest.as_str())).unwrap();
        }
        let ended = build(&home, &rule_path);
        exited(&ended, 0);
        let result: Value = serde_json::from_str(&ended.stdout).unwrap();
        assert_eq!(result["claims"].as_array().unwrap().len(), 4);
        let record = database.graph_build(&scopes, job.id).unwrap().unwrap();
        assert_eq!(record.batches.len(), 1);
        assert_eq!(
            record.batches[0].lease_number,
            if committed { 1 } else { 2 }
        );
        assert_eq!(
            database.job(&scopes, job.id).unwrap().unwrap().state,
            JobState::Succeeded
        );
        assert!(
            database
                .finish_graph_build(
                    &scopes,
                    &mut lease,
                    LeaseTiming {
                        now: UNIX_EPOCH,
                        term: Duration::from_secs(1)
                    }
                )
                .is_err()
        );
    }
}

#[test]
fn graph_resume_claim_only_build_attaches_later_without_extraction() {
    let home = Home::new();
    let rule = pilot(&home);
    let built = build(&home, &rule);
    exited(&built, 0);
    assert!(
        built
            .stdout
            .starts_with(r#"{"schema":"maestro-cli/knowledge-graph-build/1","job":""#)
    );
    let document: Value = serde_json::from_str(&built.stdout).unwrap();
    let id: ulid::Ulid = document["job"].as_str().unwrap().parse().unwrap();
    let database = home.database();
    let scopes = local(&database);
    let target = generation(&home);
    let revision = quality::eligible(&database, &scopes, COLLECTION)
        .unwrap()
        .remove(0);
    fs::remove_file(stored(&home, revision.original_digest.as_str())).unwrap();
    for _ in 0..2 {
        let attached = home.run(&[
            "knowledge",
            "graph",
            "attach",
            "--build",
            &id.to_string(),
            "--generation",
            &target.to_string(),
            "--json",
        ]);
        exited(&attached, 0);
        assert!(
            attached
                .stdout
                .starts_with(r#"{"schema":"maestro-cli/knowledge-graph-attach/1","job":""#)
        );
        let document: Value = serde_json::from_str(&attached.stdout).unwrap();
        let attach_id = document["job"].as_str().unwrap().parse().unwrap();
        assert_ne!(id, attach_id);
        assert_eq!(document["build"], id.to_string());
        assert_eq!(document["generation"], target);
        let job = database.job(&scopes, attach_id).unwrap().unwrap();
        assert_eq!(job.kind, "knowledge.graph.attach");
        assert_eq!(job.state, JobState::Succeeded);
    }
    exited(
        &home.run(&[
            "knowledge",
            "graph",
            "build",
            "--collection",
            COLLECTION,
            "--rule",
            text(&rule),
            "--generation",
            &target.to_string(),
            "--json",
        ]),
        0,
    );
    assert_eq!(
        database
            .graph_attachment(&scopes, target)
            .unwrap()
            .unwrap()
            .job,
        id
    );
    assert_eq!(
        database
            .jobs_for_resource(
                &scopes,
                "knowledge.graph.build",
                &format!("graph-build:{COLLECTION}")
            )
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        database
            .jobs_for_resource(
                &scopes,
                "knowledge.graph.attach",
                &format!("graph-attach:{target}")
            )
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn graph_resume_claim_limit_names_the_flag_and_rejection_retention_is_visible() {
    let home = Home::new();
    let rule = pilot(&home);
    let limited = home.run(&[
        "knowledge",
        "graph",
        "build",
        "--collection",
        COLLECTION,
        "--rule",
        text(&rule),
        "--max-claims",
        "3",
        "--json",
    ]);
    exited(&limited, 2);
    assert!(limited.stderr.contains("max-claims bound 3"), "{limited:?}");
    assert!(limited.stderr.contains("raise --max-claims"), "{limited:?}");
    exited(&build(&home, &rule), 0);
    let mut rejected = envelope()["rule"].clone();
    rejected["heading_path"] = json!(["absent"]);
    let rule = write(&home, "rejected.json", &rejected);
    let result = home.run(&[
        "knowledge",
        "graph",
        "build",
        "--collection",
        COLLECTION,
        "--rule",
        text(&rule),
        "--max-retained-rejections",
        "0",
        "--json",
    ]);
    exited(&result, 2);
    let output: Value = serde_json::from_str(&result.stdout).unwrap();
    assert_eq!(output["rejected"], 1);
    assert_eq!(output["retained_rejections"], 0);
    assert_eq!(output["rejections"], json!([]));
}

/// Artifact layout used by the authority store.
fn stored(home: &Home, digest: &str) -> PathBuf {
    home.data()
        .join("artifacts/sha256")
        .join(&digest[..2])
        .join(&digest[2..4])
        .join(digest)
}

#[test]
fn graph_resume_two_cli_workers_share_one_build() {
    let home = Home::new();
    let rule = pilot(&home);
    let args = [
        "knowledge",
        "graph",
        "build",
        "--collection",
        COLLECTION,
        "--rule",
        text(&rule),
        "--json",
    ];
    let first = home.start(&args);
    let second = home.start(&args);
    let first = first.finish();
    let second = second.finish();
    exited(&first, 0);
    exited(&second, 0);
    assert_eq!(first.stdout, second.stdout);
    let database = home.database();
    let scopes = local(&database);
    let jobs = database
        .jobs_for_resource(
            &scopes,
            "knowledge.graph.build",
            &format!("graph-build:{COLLECTION}"),
        )
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(
        database
            .graph_build(&scopes, jobs[0].id)
            .unwrap()
            .unwrap()
            .batches
            .len(),
        1
    );
}

#[test]
fn graph_resume_claim_budget_is_positive_and_admits_exact_fit() {
    let home = Home::new();
    let rule = pilot(&home);
    for (limit, code) in [("0", 2), ("4", 0)] {
        let result = home.run(&[
            "knowledge",
            "graph",
            "build",
            "--collection",
            COLLECTION,
            "--rule",
            text(&rule),
            "--max-claims",
            limit,
            "--json",
        ]);
        exited(&result, code);
        if limit == "0" {
            for diagnostic in ["--max-claims", "0", "1.."] {
                assert!(result.stderr.contains(diagnostic), "{result:?}");
            }
        } else {
            let document: Value = serde_json::from_str(&result.stdout).unwrap();
            assert_eq!(document["claims"].as_array().unwrap().len(), 4);
        }
    }
}

/// Import a second document with the same rule-compatible original bytes.
fn second_source(home: &Home) {
    let corpus = home.root().join("graph-root/corpus");
    fs::copy(
        corpus.join("graph/defaults.md"),
        corpus.join("graph/second.md"),
    )
    .unwrap();
    let manifest = corpus.join("maestro-corpus.jsonl");
    let first = fs::read_to_string(&manifest).unwrap();
    let mut second: Value = serde_json::from_str(first.trim()).unwrap();
    second["path"] = json!("graph/second.md");
    second["source_ref"] = json!("corpus-path:graph/second.md");
    fs::write(manifest, format!("{first}{second}\n")).unwrap();
    for command in ["import", "quality"] {
        exited(
            &home.run(&["knowledge", command, "--collection", COLLECTION]),
            0,
        );
    }
}

#[test]
fn graph_resume_committed_prefix_extracts_remaining_source_with_cumulative_budget() {
    for limit in [7, 8] {
        resume_prefix(limit);
    }
}

/// Compare a resumed two-source build with clean extraction at a cumulative bound.
fn resume_prefix(limit: usize) {
    let home = Home::new();
    let rule_path = pilot(&home);
    second_source(&home);
    let expected = clean_document(&home, &rule_path);
    let rule = TableRule::parse(&fs::read_to_string(&rule_path).unwrap()).unwrap();
    let database = home.database();
    let scopes = local(&database);
    let mut revisions = quality::eligible(&database, &scopes, COLLECTION).unwrap();
    revisions.sort_by(|left, right| left.document_id.cmp(&right.document_id));
    assert_eq!(revisions.len(), 2);
    let plan = BuildPlan {
        collection_id: COLLECTION.into(),
        provenance: rule.provenance(),
        sources: revisions
            .iter()
            .map(|revision| revision.id.clone())
            .collect(),
        budget: Budget {
            max_claims: limit,
            max_rejections: 1_000_000,
        },
    };
    let inputs = runner::inputs(&plan);
    let scope = collection_path(COLLECTION).parse().unwrap();
    let resource = format!("graph-build:{COLLECTION}");
    let new = NewJob {
        kind: "knowledge.graph.build",
        inputs: &inputs,
        scope: &scope,
        resource: Some(&resource),
    };
    let job = database.submit_job(&new, UNIX_EPOCH).unwrap();
    let mut lease = database
        .take_job(job.id, "interrupted", UNIX_EPOCH, Duration::from_secs(1))
        .unwrap();
    database.begin_graph_build(&scopes, &lease, &plan).unwrap();
    let batch = runner::extract(&database, &rule, &revisions[0], 0).unwrap();
    let receipt = database
        .record_graph_batch(
            &scopes,
            &mut lease,
            LeaseTiming {
                now: UNIX_EPOCH,
                term: Duration::from_secs(1),
            },
            &batch,
        )
        .unwrap();
    let ended = home.run(&[
        "knowledge",
        "graph",
        "build",
        "--collection",
        COLLECTION,
        "--rule",
        text(&rule_path),
        "--max-claims",
        &limit.to_string(),
        "--json",
    ]);
    let record = database.graph_build(&scopes, job.id).unwrap().unwrap();
    assert_eq!(record.batches[0], receipt);
    if limit == 7 {
        exited(&ended, 2);
        assert!(
            ended
                .stderr
                .contains("max-claims bound 7 exceeded (needed 8)"),
            "{ended:?}"
        );
        assert_eq!(record.batches.len(), 1);
        assert!(record.claim_set_id.is_none());
    } else {
        exited(&ended, 0);
        assert_eq!(record.batches.len(), 2);
        let remaining = &record.batches[1];
        assert_eq!(remaining.ordinal, 1);
        assert_eq!(remaining.revision_id, revisions[1].id);
        assert_eq!(remaining.lease_number, 2);
        assert_eq!(remaining.claims.len(), 4);
        assert_eq!((remaining.rejected, remaining.kept), (0, 0));
        let mut actual: Value = serde_json::from_str(&ended.stdout).unwrap();
        assert_eq!(actual["job"], job.id.to_string());
        actual.as_object_mut().unwrap().remove("job");
        assert_eq!(actual, expected);
    }
}

/// Clean extraction JSON, excluding its distinct job identity.
fn clean_document(home: &Home, rule: &Path) -> Value {
    let clean = build(home, rule);
    exited(&clean, 0);
    let mut expected: Value = serde_json::from_str(&clean.stdout).unwrap();
    expected.as_object_mut().unwrap().remove("job");
    assert_eq!(expected["claims"].as_array().unwrap().len(), 8);
    expected
}
