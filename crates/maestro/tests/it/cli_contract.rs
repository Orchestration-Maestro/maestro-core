//! The command line's contract (plan D12, FR-S1-012): under `--json`, one
//! JSON document per command on stdout, under a versioned schema; diagnostics
//! on stderr only; the exit codes 0 (done), 1 (the operation failed) and 2
//! (a usage error or a refused input); and a long command's job ID printed
//! before anything else, while its job still runs.

use super::support::{Home, stream, submit_synthetic_import, synthetic, types};
use maestro_kernel::{artifact::Digest, job::JobState};
use serde_json::json;
use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime},
};
use ulid::Ulid;

/// The type of the event that records a declaration added.
const ADDED: &str = "maestro.knowledge.collection.added.v1";

#[test]
fn a_command_done_prints_one_json_document_and_exits_zero() {
    let home = Home::new();
    let declaration = synthetic().join("collection.json");
    let path = declaration.to_str().unwrap();
    let added = home.run(&["--json", "knowledge", "collection", "add", path]);
    assert_eq!(
        (added.code, added.stderr.as_str()),
        (Some(0), ""),
        "{added:?}"
    );
    let digest = Digest::of(&fs::read(&declaration).unwrap());
    assert_eq!(
        added.json(),
        json!({
            "schema": "maestro-cli/collection-add/1",
            "collection": "synthetic",
            "declaration": digest.as_str(),
            "path": path,
            "sources": ["handbook"],
            "changed": true,
        })
    );
    let again = home.run(&["knowledge", "collection", "add", path, "--json"]);
    assert_eq!(
        (again.code, again.stderr.as_str()),
        (Some(0), ""),
        "{again:?}"
    );
    assert_eq!(
        again.json()["changed"],
        false,
        "the same declaration again changes nothing"
    );
    let text = home.run(&["knowledge", "collection", "add", path]);
    assert_eq!(text.code, Some(0), "{text:?}");
    assert!(
        text.stdout.starts_with(&format!(
            "collection synthetic unchanged: declaration {}",
            digest.as_str()
        )),
        "text for people without --json: {text:?}"
    );
    let journaled = stream(&home.database(), "collection/synthetic");
    assert_eq!(types(&journaled), [ADDED], "one declaration added, once");
    assert_eq!(
        journaled[0].data,
        json!({ "collection": "synthetic", "declaration": digest.as_str(), "path": path })
    );
}

#[test]
fn a_declaration_named_by_a_relative_path_is_recorded_by_its_absolute_one() {
    let home = Home::new();
    let arguments = [
        "--json",
        "knowledge",
        "collection",
        "add",
        "collection.json",
    ];
    let added = home.run_in(&synthetic(), &arguments);
    assert_eq!(added.code, Some(0), "{added:?}");
    let path = added.json()["path"].as_str().unwrap().to_owned();
    assert!(Path::new(&path).is_absolute(), "{path}");
    assert_eq!(
        fs::read(&path).unwrap(),
        fs::read(synthetic().join("collection.json")).unwrap(),
        "{path} names the declaration added"
    );
    let journaled = stream(&home.database(), "collection/synthetic");
    assert_eq!(journaled[0].data["path"], path.as_str(), "the event's too");
}

#[test]
fn a_usage_error_exits_two_with_its_diagnostic_on_stderr_only() {
    let home = Home::new();
    for arguments in [
        &["knowledge", "frobnicate", "--json"][..],
        &[],
        &["job", "wait", "not-a-job-id", "--json"],
        &["knowledge", "import", "--json"],
    ] {
        let ended = home.run(arguments);
        assert_eq!(ended.code, Some(2), "{arguments:?}: {ended:?}");
        assert_eq!(ended.stdout, "", "{arguments:?}");
        assert!(!ended.stderr.is_empty(), "{arguments:?}");
    }
    let help = home.run(&["--help"]);
    assert_eq!(help.code, Some(0), "{help:?}");
    assert!(help.stdout.contains("knowledge"), "{help:?}");
}

#[test]
fn a_refused_input_exits_two_with_its_diagnostic_on_stderr_only() {
    let home = Home::new();
    let loose = home.root().join("loose.json");
    let text = fs::read_to_string(synthetic().join("collection.json")).unwrap();
    fs::write(&loose, text.replacen('{', "{\"unknown\": 1,", 1)).unwrap();
    let missing = home.root().join("missing.json");
    let unknown_job = Ulid::generate().to_string();
    let cases: [(&[&str], &str); 5] = [
        (
            &["knowledge", "collection", "add", loose.to_str().unwrap()],
            "maestro-collection/1",
        ),
        (
            &["knowledge", "collection", "add", missing.to_str().unwrap()],
            "missing.json",
        ),
        (&["knowledge", "import", "--collection", "absent"], "absent"),
        (&["knowledge", "status", "--collection", "absent"], "absent"),
        (&["job", "wait", &unknown_job], &unknown_job),
    ];
    for (arguments, named) in cases {
        let arguments = [arguments, &["--json"]].concat();
        let ended = home.run(&arguments);
        assert_eq!(ended.code, Some(2), "{arguments:?}: {ended:?}");
        assert_eq!(ended.stdout, "", "{arguments:?}");
        assert!(
            ended.stderr.contains(named),
            "{arguments:?} names {named}: {ended:?}"
        );
    }
    assert_eq!(stream(&home.database(), "collection/synthetic"), []);
}

#[test]
fn a_collection_the_local_principal_cannot_read_is_refused() {
    let home = Home::new();
    home.configure("[access]\nread = ['workspace/default/collection/other']\n");
    let declaration = synthetic().join("collection.json");
    let refused = home.run(&[
        "knowledge",
        "collection",
        "add",
        declaration.to_str().unwrap(),
    ]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(refused.stdout, "");
    assert!(
        refused
            .stderr
            .contains("workspace/default/collection/synthetic")
            && refused.stderr.contains("config.toml"),
        "names the scope and where to grant it: {refused:?}"
    );
}

#[test]
fn an_operation_that_fails_exits_one_with_its_diagnostic_on_stderr_only() {
    let home = Home::new();
    let data = home.data();
    fs::remove_dir(&data).unwrap();
    fs::write(&data, "not a directory").unwrap();
    let failed = home.run(&["knowledge", "status", "--collection", "synthetic", "--json"]);
    assert_eq!(failed.code, Some(1), "{failed:?}");
    assert_eq!(failed.stdout, "");
    assert!(
        failed.stderr.contains("kernel.sqlite3: "),
        "the file that failed, then why: {failed:?}"
    );
}

#[test]
fn a_long_command_prints_its_job_id_before_anything_else() {
    let home = Home::new();
    home.add_synthetic();
    let mut running = home.start(&["knowledge", "import", "--collection", "synthetic"]);
    let first = running.line();
    let id = first.strip_prefix("job ").unwrap();
    let id: Ulid = id.parse().unwrap();
    let ended = running.finish();
    assert_eq!(ended.code, Some(0), "{ended:?}");
    let json = home.run(&["knowledge", "import", "--collection", "synthetic", "--json"]);
    assert_eq!(json.code, Some(0), "{json:?}");
    assert_eq!(
        json.stderr.lines().next(),
        Some(format!("job {id}").as_str()),
        "under --json the job ID is stderr's first line: {json:?}"
    );
    assert!(
        json.stdout.starts_with(&format!(
            "{{\"schema\":\"maestro-cli/import/1\",\"job\":\"{id}\","
        )),
        "and the document's first member after its schema: {json:?}"
    );
}

#[test]
fn under_json_the_job_id_comes_on_stderr_while_the_job_still_runs() {
    let home = Home::new();
    home.add_synthetic();
    let database = home.database();
    let job = submit_synthetic_import(&database);
    let lease = database
        .take_job(
            job.id,
            "another",
            SystemTime::now(),
            Duration::from_secs(60),
        )
        .unwrap();
    let mut running = home.start(&["knowledge", "import", "--collection", "synthetic", "--json"]);
    // Another process holds the job for a minute: the import follows it.
    assert_eq!(running.error_line(), format!("job {}", job.id));
    let outcome = json!({ "collection": "synthetic", "imported": 0 });
    database
        .complete_job(&lease, JobState::Succeeded, &outcome)
        .unwrap();
    let ended = running.finish();
    assert_eq!(ended.code, Some(0), "{ended:?}");
    assert_eq!(ended.stderr, format!("job {}\n", job.id), "{ended:?}");
    assert_eq!(ended.json()["outcome"], outcome, "{ended:?}");
}
