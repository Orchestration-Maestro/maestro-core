//! Exact preview/apply grammar and outcomes; cleanup needs no native engine.
use super::{
    graph_build::exited,
    graph_cleanup_support::{document, guards, retired},
    support::Home,
};
use maestro_canonicalization::{ControlFile, OwnedRoot};
#[cfg(unix)]
use maestro_kernel::{
    facts::ClaimSetRecord,
    job::{SUCCEEDED, stream},
    journal::{Event, Filter},
};
use maestro_kernel::{job::JobState, scope::LOCAL};
use serde_json::json;
#[cfg(unix)]
use std::fmt::Write as _;
use std::fs;

#[test]
fn graph_cleanup_unknown_and_denied_have_exact_same_refusal_and_create_nothing() {
    let home = Home::new();
    guards(&home);
    let ended = home.run(&[
        "--json",
        "knowledge",
        "graph",
        "cleanup",
        "--generation",
        "123",
    ]);
    exited(&ended, 2);
    assert_eq!(
        ended.json(),
        json!({
            "schema": "maestro-cli/knowledge-graph-cleanup/1",
            "action": "refused", "reason": "target_unavailable", "generation": 123
        })
    );
    assert_eq!(fs::read_dir(home.data().join("graph")).unwrap().count(), 2);
    let (generation, _) = retired(&home);
    home.configure("[access]\nread = []\n");
    let denied = home.run(&[
        "--json",
        "knowledge",
        "graph",
        "cleanup",
        "--generation",
        &generation.to_string(),
    ]);
    exited(&denied, 2);
    let mut expected = ended.json();
    expected["generation"] = json!(generation);
    assert_eq!(denied.json(), expected);
    assert!(
        denied
            .stderr
            .contains("unknown or unauthorized graph cleanup target")
    );
}

#[test]
fn graph_cleanup_preview_exact_json_and_text_never_write_or_remove() {
    let home = Home::new();
    let (generation, name) = retired(&home);
    let database = home.database();
    let receipt = database
        .projection_ready(&database.visible(LOCAL).unwrap(), generation)
        .unwrap();
    let preview = home.run(&[
        "--json",
        "knowledge",
        "graph",
        "cleanup",
        "--generation",
        &generation.to_string(),
    ]);
    exited(&preview, 0);
    assert!(
        preview
            .stdout
            .starts_with(r#"{"schema":"maestro-cli/knowledge-graph-cleanup/1","#)
    );
    assert_eq!(
        preview.json(),
        document(generation, &name, "preview", "candidate", true)
    );
    assert_eq!(preview.stderr, "");
    let plain = home.run(&[
        "knowledge",
        "graph",
        "cleanup",
        "--generation",
        &generation.to_string(),
    ]);
    exited(&plain, 0);
    assert_eq!(
        plain.stdout,
        format!(
            "graph cleanup preview: generation {generation}, file {name}, present; \
             repeat with --yes to apply\n"
        )
    );
    assert_eq!(
        database
            .projection_ready(&database.visible(LOCAL).unwrap(), generation)
            .unwrap(),
        receipt
    );
    assert_eq!(
        fs::read(home.data().join("graph").join(name)).unwrap(),
        b"disposable"
    );
}

#[cfg(unix)]
#[test]
fn graph_cleanup_apply_exact_json_and_repeat_preserve_claims_receipt_and_neighbours() {
    let home = Home::new();
    let (generation, name) = retired(&home);
    let graph = home.data().join("graph");
    fs::write(graph.join("unrelated.wal"), b"keep byte for byte").unwrap();
    let before = claim_authority(&home, generation);
    let applied = home.run(&[
        "--json",
        "knowledge",
        "graph",
        "cleanup",
        "--generation",
        &generation.to_string(),
        "--yes",
    ]);
    exited(&applied, 0);
    let mut expected = document(generation, &name, "applied", "removed", false);
    expected["job"] = applied.json()["job"].clone();
    assert_eq!(applied.json(), expected);
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    let id = applied.json()["job"].as_str().unwrap().parse().unwrap();
    let job = database.job(&scopes, id).unwrap().unwrap();
    assert_eq!(job.state, JobState::Succeeded);
    assert_eq!(job.kind, "knowledge.graph.cleanup");
    assert_eq!(job.outcome, Some(json!({"reason": "removed"})));
    let receipt = database
        .projection_ready(&scopes, generation)
        .unwrap()
        .unwrap();
    assert_eq!(receipt.file_name, name);
    assert_eq!(claim_authority(&home, generation), before);
    let retry = home.run(&[
        "--json",
        "knowledge",
        "graph",
        "cleanup",
        "--generation",
        &generation.to_string(),
        "--yes",
    ]);
    exited(&retry, 0);
    expected["reason"] = json!("already_missing");
    assert_eq!(retry.json(), expected);
    let plain = home.run(&[
        "knowledge",
        "graph",
        "cleanup",
        "--generation",
        &generation.to_string(),
        "--yes",
    ]);
    exited(&plain, 0);
    let events = database
        .events(
            &scopes,
            &Filter {
                stream: &stream(id),
                after: 0,
                r#type: None,
            },
        )
        .unwrap();
    assert_eq!(plain.stdout, followed_text(events, id, generation, &name));
    assert_eq!(
        fs::read(graph.join("unrelated.wal")).unwrap(),
        b"keep byte for byte"
    );
    let preview = home.run(&[
        "--json",
        "knowledge",
        "graph",
        "cleanup",
        "--generation",
        &generation.to_string(),
    ]);
    assert_eq!(
        preview.json(),
        document(generation, &name, "preview", "already_missing", false)
    );
}

/// Whole claim records, including their original provenance and supports, must be retained.
#[cfg(unix)]
fn claim_authority(home: &Home, generation: i64) -> ClaimSetRecord {
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    let receipt = database
        .projection_ready(&scopes, generation)
        .unwrap()
        .unwrap();
    let claims = database
        .claim_set(&scopes, &receipt.claim_set_id)
        .unwrap()
        .unwrap();
    assert_eq!(claims.claims.len(), 4);
    claims
}

/// The existing foreground follower strips terminal outcomes from journal text.
#[cfg(unix)]
fn followed_text(events: Vec<Event>, id: ulid::Ulid, generation: i64, name: &str) -> String {
    assert_eq!(events.len(), 3);
    let mut lines = format!("job {id}\n");
    for mut event in events {
        if event.r#type == SUCCEEDED {
            event.data.as_object_mut().unwrap().remove("outcome");
        }
        writeln!(lines, "{} {} {}", event.sequence, event.r#type, event.data).unwrap();
    }
    writeln!(
        lines,
        "graph cleanup applied: generation {generation}, file {name}, already_missing"
    )
    .unwrap();
    lines
}

#[test]
fn graph_cleanup_requires_generation_and_accepts_only_setup_confirmation_grammar() {
    let home = Home::new();
    for arguments in [
        vec!["knowledge", "graph", "cleanup"],
        vec![
            "knowledge",
            "graph",
            "cleanup",
            "--generation",
            "1",
            "--apply",
        ],
        vec![
            "knowledge",
            "graph",
            "cleanup",
            "--generation",
            "1",
            "--file",
            "guessed.lbdb",
        ],
    ] {
        exited(&home.run(&arguments), 2);
    }
    assert!(!home.data().join("graph").exists());
}

#[test]
fn graph_cleanup_busy_and_missing_guards_have_exact_refusals_with_valid_neighbour() {
    use maestro_canonicalization::{LockMode, SystemFileLock};
    let home = Home::new();
    let (generation, name) = retired(&home);
    let graph = home.data().join("graph");
    let root = OwnedRoot::open(&graph, false).unwrap();
    let reader = root.open_control(ControlFile::Access).unwrap();
    reader
        .lock_with(&SystemFileLock, LockMode::Shared, false)
        .unwrap();
    let busy = home.run(&[
        "--json",
        "knowledge",
        "graph",
        "cleanup",
        "--generation",
        &generation.to_string(),
        "--yes",
    ]);
    exited(&busy, 2);
    assert_eq!(
        busy.json(),
        json!({
            "schema": "maestro-cli/knowledge-graph-cleanup/1",
            "action": "refused", "reason": "access_busy", "generation": generation
        })
    );
    drop(reader);
    fs::remove_file(graph.join(".access.guard")).unwrap();
    let missing = home.run(&[
        "--json",
        "knowledge",
        "graph",
        "cleanup",
        "--generation",
        &generation.to_string(),
    ]);
    exited(&missing, 2);
    assert_eq!(
        missing.json(),
        json!({
            "schema": "maestro-cli/knowledge-graph-cleanup/1",
            "action": "refused", "reason": "guard_missing", "generation": generation
        })
    );
    assert!(!graph.join(".access.guard").exists());
    assert!(root.ensure_control(ControlFile::Access).unwrap());
    let preview = home.run(&[
        "--json",
        "knowledge",
        "graph",
        "cleanup",
        "--generation",
        &generation.to_string(),
    ]);
    exited(&preview, 0);
    assert_eq!(
        preview.json(),
        document(generation, &name, "preview", "candidate", true)
    );
}

#[cfg(windows)]
#[test]
fn graph_cleanup_windows_unsupported_apply_keeps_file_and_records_fixed_job_reason() {
    let home = Home::new();
    let (generation, name) = retired(&home);
    let applied = home.run(&[
        "--json",
        "knowledge",
        "graph",
        "cleanup",
        "--generation",
        &generation.to_string(),
        "--yes",
    ]);
    exited(&applied, 2);
    assert_eq!(
        applied.json(),
        json!({
            "schema": "maestro-cli/knowledge-graph-cleanup/1",
            "action": "refused", "reason": "unsupported", "generation": generation
        })
    );
    assert_eq!(
        fs::read(home.data().join("graph").join(name)).unwrap(),
        b"disposable"
    );
    let job = applied
        .stderr
        .lines()
        .find_map(|line| line.strip_prefix("job "))
        .unwrap()
        .parse()
        .unwrap();
    let database = home.database();
    let job = database
        .job(&database.visible(LOCAL).unwrap(), job)
        .unwrap()
        .unwrap();
    assert_eq!(job.state, JobState::Failed);
    assert_eq!(job.outcome, Some(json!({"reason": "unsupported"})));
}
