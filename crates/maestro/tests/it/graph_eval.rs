//! Graph evaluation refuses unsafe input without opening a default kernel.

use super::support::Home;
use std::{fs, path::PathBuf};

#[test]
fn graph_eval_malformed_manifest_is_sanitized_before_kernel_open() {
    let home = Home::bare();
    let path = home.root().join("manifest.json");
    fs::write(&path, r#"{"private-question-quote-sentinel": "secret"}"#).unwrap();
    let result = home.run(&[
        "eval",
        "graph",
        "check",
        "--manifest",
        path.to_str().unwrap(),
    ]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(
        result.stderr.contains("graph_manifest_invalid"),
        "{result:?}"
    );
    assert!(!result.stderr.contains("private-question-quote-sentinel"));
    assert!(result.stdout.is_empty());
    assert!(!home.data().join("kernel.sqlite3").exists());
}

#[test]
fn graph_eval_private_ladder_parse_error_never_echoes_private_text() {
    let home = Home::bare();
    let path = home.root().join("manifest.json");
    fs::write(
        &path,
        r#"{"schema":"maestro-ladder-manifest/1","private-question-quote-sentinel":0}"#,
    )
    .unwrap();
    let result = home.run(&["eval", "ladder", "--manifest", path.to_str().unwrap()]);
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(
        !result.stderr.contains("private-question-quote-sentinel"),
        "{result:?}"
    );
    assert!(!home.data().join("kernel.sqlite3").exists());
}

/// Writes a fully isolated synthetic check and returns its manifest path.
pub(super) fn manifest(home: &Home) -> PathBuf {
    use maestro_kernel::artifact::Digest;
    use serde_json::json;
    let private = home.root().join("private");
    for directory in [
        "private",
        "live-data",
        "live-config",
        "scratch-storage",
        "live-storage",
    ] {
        fs::create_dir(home.root().join(directory)).unwrap();
    }
    let suite = json!({"schema":"maestro-suite/1", "id":"q1", "language":"en",
        "question":"private-question-quote-sentinel", "answerable":false, "expected":[]})
    .to_string();
    let labels = json!({"schema":"maestro-graph-labels/1", "id":"q1", "family":"f1",
        "kind":"unanswerable", "language":"en", "proofs":[],
        "unanswerable_reason":"private-question-quote-sentinel"})
    .to_string();
    fs::write(private.join("suite.jsonl"), &suite).unwrap();
    fs::write(private.join("labels.jsonl"), &labels).unwrap();
    let value = json!({
        "schema":"maestro-graph-check/1", "suite":"suite.jsonl", "labels":"labels.jsonl",
        "suite_digest":Digest::of(suite.as_bytes()).as_str(),
        "labels_digest":Digest::of(labels.as_bytes()).as_str(),
        "stage":"draft",
        "collection":"synthetic",
        "private_run": {
            "scratch_data":home.root().join("data"), "scratch_config":home.root().join("config"),
            "live_data":home.root().join("live-data"),
        "live_config":home.root().join("live-config"),
            "scratch_storage":home.root().join("scratch-storage"),
        "live_storage":home.root().join("live-storage"),
            "scratch_endpoint":"http://127.0.0.1:16334", "live_endpoint":"http://127.0.0.1:6334",
            "private_root":private, "output":private,
            "backup_id":"backup-1", "backup_digest":Digest::of(b"backup").as_str(),
            "approval":{"scope":"synthetic",
        "target":"graph-evaluation",
        "expires_unix":4_102_444_800_u64,
                "evidence_digest":Digest::of(b"approval").as_str()}
        }
    });
    let path = private.join("manifest.json");
    fs::write(&path, value.to_string()).unwrap();
    path
}

#[test]
fn graph_eval_check_is_inference_free_and_prints_only_counts_and_digests() {
    let home = Home::bare();
    let path = manifest(&home);
    let mut command = home.command(&[
        "--json",
        "eval",
        "graph",
        "check",
        "--manifest",
        path.to_str().unwrap(),
    ]);
    command.env("MAESTRO_QDRANT_URL", "http://127.0.0.1:16334");
    let result = command.output().unwrap();
    assert!(result.status.success(), "{result:?}");
    let text = String::from_utf8(result.stdout).unwrap();
    assert!(!text.contains("private-question-quote-sentinel"));
    let value: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(value["items"], 1);
    assert_eq!(value["m2"], "blocked_g13_g27_g32");
    assert_eq!(value["unreviewed"], 1);
    assert_eq!(value["unanswerable"], 1);
    assert!(result.stderr.is_empty());
    assert!(
        !home
            .root()
            .join("live-data/maestro/kernel.sqlite3")
            .exists()
    );
}

#[test]
fn graph_eval_refuses_missing_overlapping_and_expired_bindings_before_open() {
    for defect in ["missing", "overlap", "expired", "endpoint", "digest"] {
        let home = Home::bare();
        let path = manifest(&home);
        let mut value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        match defect {
            "missing" => {
                value["private_run"]
                    .as_object_mut()
                    .unwrap()
                    .remove("scratch_data");
            }
            "overlap" => {
                value["private_run"]["live_data"] = value["private_run"]["scratch_data"].clone();
            }
            "expired" => value["private_run"]["approval"]["expires_unix"] = 0.into(),
            "endpoint" => value["private_run"]["live_endpoint"] = "http://localhost:16334/".into(),
            "digest" => value["suite_digest"] = "invalid".into(),
            _ => panic!("unknown defect"),
        }
        fs::write(&path, value.to_string()).unwrap();
        let result = home
            .command(&[
                "eval",
                "graph",
                "check",
                "--manifest",
                path.to_str().unwrap(),
            ])
            .env("MAESTRO_QDRANT_URL", "http://127.0.0.1:16334")
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(2), "{defect}: {result:?}");
        assert!(!home.data().join("kernel.sqlite3").exists(), "{defect}");
        assert!(
            !String::from_utf8_lossy(&result.stderr).contains("private-question-quote-sentinel")
        );
    }
}

/// A private ladder uses the existing runner, with graph-check inputs bound by digest.
pub(super) fn ladder(home: &Home, graph: &PathBuf, route: &str) -> PathBuf {
    use serde_json::json;
    let output = home.root().join("private/results");
    fs::create_dir(&output).unwrap();
    let value = json!({"schema":"maestro-ladder-manifest/1", "suite":"suite.jsonl",
        "collection":"synthetic", "warm_ups":0, "output":output,"graph_manifest":graph,
        "rungs":[{"name":"passage-only", "ask":false,"configuration":{
            "routes":{"dense":true,"lexical":true,"identifier":true,"structured":true,
                "graph":route}, "rrf_k":60,
            "weights":{"dense":1.0,"lexical":1.0,"identifier":1.0,"structured":1.0},
            "rerank":null}}]});
    let path = home.root().join("private/ladder.json");
    fs::write(&path, value.to_string()).unwrap();
    path
}

#[test]
fn graph_eval_graph_enabled_rung_is_explicitly_unavailable_before_open() {
    let home = Home::bare();
    let graph = manifest(&home);
    let path = ladder(&home, &graph, "ladybug");
    let result = home
        .command(&["eval", "ladder", "--manifest", path.to_str().unwrap()])
        .env("MAESTRO_QDRANT_URL", "http://127.0.0.1:16334")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("graph_route_not_available_until_g13_g27"),
        "{result:?}"
    );
    assert!(!home.data().join("kernel.sqlite3").exists());
}

#[test]
fn graph_eval_private_ladder_retains_raw_failure_without_printing_it() {
    let home = Home::bare();
    let graph = manifest(&home);
    let path = ladder(&home, &graph, "none");
    let result = home
        .command(&["eval", "ladder", "--manifest", path.to_str().unwrap()])
        .env("MAESTRO_QDRANT_URL", "http://127.0.0.1:16334")
        .env("MAESTRO_ROUTER_URL", "http://127.0.0.1:18000")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr).trim(),
        "graph_ladder_failed"
    );
    assert!(result.stdout.is_empty());
    let receipts = fs::read_dir(home.root().join("private"))
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("diagnostic-")
        })
        .count();
    assert_eq!(receipts, 1);
}

#[test]
fn graph_eval_anchors_resolve_only_against_scoped_scratch_authority() {
    use maestro_kernel::{
        artifact::Digest,
        document::{Document, Revision, RevisionStatus},
    };
    use serde_json::{Map, Value, json};
    let home = Home::new();
    home.add_synthetic();
    let path = manifest(&home);
    let source = b"exact private source text";
    let database = home.database();
    database
        .record_document(&Document {
            id: "graph-document".to_owned(),
            collection_id: "synthetic".to_owned(),
            source_id: "handbook".to_owned(),
            source_ref: "source:graph".to_owned(),
        })
        .unwrap();
    let original = database.put(source, "text/plain").unwrap();
    database
        .record_revision(&Revision {
            id: "graph-revision".to_owned(),
            document_id: "graph-document".to_owned(),
            original_digest: original.clone(),
            canonical_digest: database.put(b"{}", "application/json").unwrap(),
            status: RevisionStatus::Valid,
            captured_at: None,
            metadata: Map::new(),
        })
        .unwrap();
    let suite = json!({"schema":"maestro-suite/1","id":"q1","language":"en",
        "question":"private-question-quote-sentinel","answerable":true,
        "expected":[{"source_ref":"source:graph","heading_path":[]}]})
    .to_string();
    let label = json!({"schema":"maestro-graph-labels/1","id":"q1","family":"f1",
        "kind":"relationship","language":"en","proofs":[{"links":[{
            "subject":"parameter","predicate":"DEFAULTS_TO","object":"3","conditions":[],
            "anchors":[{"source_ref":"source:graph","original":original.as_str(),
                "span":[0,5],"quote":Digest::of(b"exact").as_str()}]}]}]})
    .to_string();
    let private = home.root().join("private");
    fs::write(private.join("suite.jsonl"), &suite).unwrap();
    fs::write(private.join("labels.jsonl"), &label).unwrap();
    let mut value: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    value["suite_digest"] = Digest::of(suite.as_bytes()).as_str().into();
    value["labels_digest"] = Digest::of(label.as_bytes()).as_str().into();
    fs::write(&path, value.to_string()).unwrap();
    let run = || {
        home.command(&[
            "--json",
            "eval",
            "graph",
            "check",
            "--manifest",
            path.to_str().unwrap(),
        ])
        .env("MAESTRO_QDRANT_URL", "http://127.0.0.1:16334")
        .output()
        .unwrap()
    };
    let result = run();
    assert!(result.status.success(), "{result:?}");
    let summary: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(summary["anchors"], 1);
    assert_eq!(summary["answerable"], 1);
    assert_eq!(summary["links"], 1);
    assert!(!String::from_utf8_lossy(&result.stdout).contains("private-question-quote-sentinel"));
    home.configure("[access]\nread = []\n");
    let refused = run();
    assert_eq!(refused.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&refused.stderr).trim(),
        "graph_labels_invalid"
    );
    assert!(refused.stdout.is_empty());
}

#[test]
fn graph_eval_ladder_isolation_keeps_safe_code() {
    let home = Home::bare();
    let graph = manifest(&home);
    let path = ladder(&home, &graph, "none");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&graph).unwrap()).unwrap();
    value["private_run"]["approval"]["expires_unix"] = 0.into();
    fs::write(&graph, value.to_string()).unwrap();
    let result = home
        .command(&["eval", "ladder", "--manifest", path.to_str().unwrap()])
        .env("MAESTRO_QDRANT_URL", "http://127.0.0.1:16334")
        .env("MAESTRO_ROUTER_URL", "http://localhost:18000")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr).trim(),
        "graph_scratch_refused"
    );
}

#[test]
fn graph_eval_public_ladder_operational_failure_exits_one() {
    let home = Home::bare();
    let graph = manifest(&home);
    let path = ladder(&home, &graph, "none");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    value.as_object_mut().unwrap().remove("graph_manifest");
    fs::write(&path, value.to_string()).unwrap();
    fs::create_dir(home.data().join("kernel.sqlite3")).unwrap();
    let result = home.run(&["eval", "ladder", "--manifest", path.to_str().unwrap()]);
    assert_eq!(result.code, Some(1), "{result:?}");
    assert_eq!(result.stderr.trim(), "graph_ladder_failed");
}

#[test]
fn graph_eval_relative_ladder_manifest_resolves_output() {
    let home = Home::bare();
    let graph = manifest(&home);
    let path = ladder(&home, &graph, "none");
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    value["output"] = "results".into();
    value["graph_manifest"] = "manifest.json".into();
    fs::write(&path, value.to_string()).unwrap();
    let result = home
        .command(&["eval", "ladder", "--manifest", "private/ladder.json"])
        .current_dir(home.root())
        .env("MAESTRO_QDRANT_URL", "http://localhost:16334")
        .env("MAESTRO_ROUTER_URL", "http://localhost:18000")
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&result.stderr).trim(),
        "graph_ladder_failed"
    );
    assert!(home.data().join("kernel.sqlite3").exists());
}

#[test]
fn graph_eval_frozen_unreviewed_labels_are_refused() {
    let home = Home::bare();
    let path = manifest(&home);
    let mut value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    value["stage"] = "frozen".into();
    fs::write(&path, value.to_string()).unwrap();
    let result = home
        .command(&[
            "eval",
            "graph",
            "check",
            "--manifest",
            path.to_str().unwrap(),
        ])
        .env("MAESTRO_QDRANT_URL", "http://localhost:16334")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr).trim(),
        "graph_labels_invalid"
    );
}

#[test]
fn graph_eval_authority_open_failure_exits_one_without_private_text() {
    let home = Home::bare();
    let path = manifest(&home);
    fs::write(
        home.data().join("kernel.sqlite3"),
        b"not sqlite private-question-quote-sentinel",
    )
    .unwrap();
    let result = home
        .command(&[
            "eval",
            "graph",
            "check",
            "--manifest",
            path.to_str().unwrap(),
        ])
        .env("MAESTRO_QDRANT_URL", "http://localhost:16334")
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert_eq!(
        String::from_utf8_lossy(&result.stderr).trim(),
        "graph_authority_failed"
    );
    assert!(result.stdout.is_empty());
}
