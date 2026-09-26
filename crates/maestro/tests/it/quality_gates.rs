//! `knowledge quality`: the quality gate (T020) over a collection, run as a
//! leased job of the collection's scope in the foreground, holding the
//! collection's quality gate: its report printed as the job ended, a rerun
//! finding the same job, a gate after an import a new job, the ledger beside
//! the declaration deciding first, a gate that fails exiting 1, and the
//! refusals exiting 2.

use super::support::{Ended, Home, local, stream, synthetic, types};
use maestro_kernel::{
    artifact::Digest,
    job::{JobState, NewJob},
};
use serde_json::{Value, json};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime},
};
use ulid::Ulid;

/// The kind of the job `knowledge quality` runs.
const QUALITY: &str = "knowledge.quality";
/// The resource a gate of `synthetic` holds.
const SYNTHETIC_QUALITY: &str = "collection/synthetic/quality";
/// A ledger rule that excludes the synthetic collection's backup policy.
const RULE: &str = concat!(
    r#"{"schema": "maestro-quality-ledger/1", "id": "retired-policy", "#,
    r#""match": {"source_ref": "https://handbook.example.org/4.2/backups/backup-policy"}, "#,
    r#""disposition": "excluded", "reason": "the policy was retired", "#,
    r#""decided_by": "Ada", "reversal": "remove this rule"}"#,
);

/// The job the binary ran or found under `--json`: stderr's first line.
fn job_of(ended: &Ended) -> Ulid {
    let first = ended.stderr.lines().next().unwrap_or_default();
    first
        .strip_prefix("job ")
        .unwrap_or_else(|| panic!("no job first: {ended:?}"))
        .parse()
        .unwrap()
}

/// Runs `knowledge quality --collection synthetic --json` in `home`.
fn gate(home: &Home) -> Ended {
    home.run(&[
        "knowledge",
        "quality",
        "--collection",
        "synthetic",
        "--json",
    ])
}

/// Imports the synthetic collection in `home`, once it was added.
fn import(home: &Home) {
    let imported = home.run(&["knowledge", "import", "--collection", "synthetic"]);
    assert_eq!(imported.code, Some(0), "{imported:?}");
}

/// Adds, in `home`, the synthetic declaration copied beside a quality ledger
/// that holds `ledger`, as the declaration names it: `quality/ledger.jsonl`.
fn add_beside_ledger(home: &Home, ledger: &str) -> PathBuf {
    let directory = home.root().join("declared");
    fs::create_dir_all(directory.join("quality")).unwrap();
    let declaration = directory.join("collection.json");
    fs::copy(synthetic().join("collection.json"), &declaration).unwrap();
    fs::write(directory.join("quality").join("ledger.jsonl"), ledger).unwrap();
    let added = home.run(&[
        "knowledge",
        "collection",
        "add",
        declaration.to_str().unwrap(),
    ]);
    assert_eq!(added.code, Some(0), "{added:?}");
    directory.join("quality").join("ledger.jsonl")
}

/// The outcomes of a report: `accepted` of them accepted, `excluded` excluded.
fn outcomes(accepted: u64, excluded: u64) -> Value {
    json!({
        "accepted": accepted,
        "accepted_with_warnings": 0,
        "needs_reextraction": 0,
        "quarantined": 0,
        "excluded": excluded,
    })
}

#[test]
fn the_gate_decides_every_revision_as_a_job_and_prints_its_report() {
    let home = Home::new();
    home.add_synthetic();
    import(&home);
    let gated = gate(&home);
    assert_eq!(gated.code, Some(0), "{gated:?}");
    let id = job_of(&gated);
    assert_eq!(
        gated.json(),
        json!({
            "schema": "maestro-cli/knowledge-quality/1",
            "job": id.to_string(),
            "kind": QUALITY,
            "attempt": 1,
            "state": "succeeded",
            "outcome": {
                "collection": "synthetic",
                "revisions": 28,
                "decided": 28,
                "kept": 0,
                "outcomes": outcomes(28, 0),
                "rules": {},
                "held": [],
            },
        })
    );
    let database = home.database();
    let scopes = local(&database);
    let job = database.job(&scopes, id).unwrap().unwrap();
    assert_eq!(job.scope.as_str(), "workspace/default/collection/synthetic");
    assert_eq!(job.resource.as_deref(), Some(SYNTHETIC_QUALITY));
    let events = stream(&database, &format!("job/{id}"));
    assert_eq!(
        types(&events),
        [
            "maestro.job.created.v1",
            "maestro.job.taken.v1",
            "maestro.job.succeeded.v1",
        ]
    );
    let mut revisions = String::new();
    for revision in database.revisions(&scopes, "synthetic").unwrap() {
        revisions.push_str(&revision.id);
        revisions.push('\n');
    }
    let declaration = fs::read(synthetic().join("collection.json")).unwrap();
    assert_eq!(
        events[0].data,
        json!({ "inputs": {
            "collection": "synthetic",
            "declaration": Digest::of(&declaration).as_str(),
            "ledger": null,
            "revisions": Digest::of(revisions.as_bytes()).as_str(),
        }}),
        "the ledger the declaration names is not there, so it is none"
    );
    let holder = events[1].data["lease"]["holder"].as_str().unwrap();
    assert!(holder.starts_with("maestro-cli/"), "{holder}");
    let counts = database.collection_counts(&scopes, "synthetic").unwrap();
    assert_eq!(counts.undecided, 0, "every revision decided: {counts:?}");
    let again = home.run(&["knowledge", "quality", "--collection", "synthetic"]);
    assert_eq!(again.code, Some(0), "{again:?}");
    let lines: Vec<&str> = again.stdout.lines().collect();
    assert_eq!(
        lines.first().copied(),
        Some(format!("job {id}").as_str()),
        "the same inputs find the same job: {again:?}"
    );
    assert!(
        lines
            .last()
            .is_some_and(|line| line.starts_with("succeeded {")),
        "printed as it ended, in text: {again:?}"
    );
}

#[test]
fn a_gate_after_an_import_is_a_new_job_that_decides_what_it_imported() {
    let home = Home::new();
    home.add_synthetic();
    let before = gate(&home);
    assert_eq!(before.code, Some(0), "{before:?}");
    assert_eq!(before.json()["outcome"]["revisions"], 0);
    import(&home);
    let after = gate(&home);
    assert_eq!(after.code, Some(0), "{after:?}");
    assert_ne!(job_of(&after), job_of(&before), "new revisions, a new job");
    let document = after.json();
    assert_eq!(document["attempt"], 1, "a job of another key: {document}");
    assert_eq!(document["outcome"]["decided"], 28, "{document}");
}

#[test]
fn the_ledger_beside_the_declaration_decides_before_the_checks() {
    let home = Home::new();
    add_beside_ledger(&home, &format!("{RULE}\n"));
    import(&home);
    let gated = gate(&home);
    assert_eq!(gated.code, Some(0), "{gated:?}");
    let document = gated.json();
    let outcome = &document["outcome"];
    assert_eq!(outcome["outcomes"], outcomes(27, 1), "{document}");
    assert_eq!(outcome["rules"], json!({ "ledger.retired-policy": 1 }));
    let held = outcome["held"].as_array().unwrap();
    assert_eq!(held.len(), 1, "{document}");
    assert_eq!(
        (
            &held[0]["source_ref"],
            &held[0]["outcome"],
            &held[0]["decided_by"],
            &held[0]["reasons"],
        ),
        (
            &json!("https://handbook.example.org/4.2/backups/backup-policy"),
            &json!("excluded"),
            &json!("Ada"),
            &json!(["the policy was retired"]),
        )
    );
    let database = home.database();
    let events = stream(&database, &format!("job/{}", job_of(&gated)));
    let ledger = format!("{RULE}\n");
    assert_eq!(
        events[0].data["inputs"]["ledger"],
        Digest::of(ledger.as_bytes()).as_str(),
        "the ledger's digest is a frozen input"
    );
}

#[test]
fn a_ledger_that_is_not_strict_is_refused_before_any_job() {
    let home = Home::new();
    let ledger = add_beside_ledger(&home, "{\"schema\": \"maestro-quality-ledger/1\"}\n");
    let refused = gate(&home);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(refused.stdout, "");
    assert!(
        refused.stderr.contains(&ledger.display().to_string())
            && refused.stderr.contains("line 1 of the quality ledger"),
        "names the ledger and its line: {refused:?}"
    );
    assert!(
        !refused.stderr.starts_with("job "),
        "refused before any job: {refused:?}"
    );
}

#[test]
fn a_ledger_that_cannot_be_read_is_refused_rather_than_read_as_empty() {
    let home = Home::new();
    let ledger = add_beside_ledger(&home, "");
    fs::remove_file(&ledger).unwrap();
    fs::create_dir(&ledger).unwrap();
    let refused = gate(&home);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(refused.stdout, "");
    assert!(
        refused.stderr.contains(&ledger.display().to_string()),
        "names the ledger: {refused:?}"
    );
    assert!(
        !refused.stderr.starts_with("job "),
        "refused before any job: {refused:?}"
    );
}

#[test]
fn a_collection_never_added_is_refused() {
    let home = Home::new();
    let refused = home.run(&["knowledge", "quality", "--collection", "absent", "--json"]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(refused.stdout, "");
    assert!(refused.stderr.contains("absent"), "{refused:?}");
}

#[test]
fn a_gate_that_fails_ends_its_job_failed_and_exits_one() {
    let home = Home::new();
    home.add_synthetic();
    import(&home);
    let database = home.database();
    let revisions = database.revisions(&local(&database), "synthetic").unwrap();
    let lost = revisions[0].canonical_digest.as_str().to_owned();
    fs::remove_file(
        home.data()
            .join("artifacts")
            .join("sha256")
            .join(&lost[..2])
            .join(&lost[2..4])
            .join(&lost),
    )
    .unwrap();
    let failed = gate(&home);
    assert_eq!(failed.code, Some(1), "{failed:?}");
    let document = failed.json();
    assert_eq!(document["state"], "failed", "{document}");
    assert!(
        document["outcome"]["error"]
            .as_str()
            .is_some_and(|error| error.contains(&lost)),
        "names the artifact it could not read: {document}"
    );
    let again = gate(&home);
    assert_eq!(again.code, Some(1), "{again:?}");
    assert_eq!(
        again.json()["attempt"],
        2,
        "a failed job's key starts the next attempt"
    );
}

#[test]
fn a_gate_whose_resource_a_live_lease_holds_is_refused_naming_its_job() {
    let home = Home::new();
    home.add_synthetic();
    let database = home.database();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    let inputs = json!({ "collection": "synthetic", "revisions": "older" });
    let new = NewJob {
        kind: QUALITY,
        inputs: &inputs,
        scope: &scope,
        resource: Some(SYNTHETIC_QUALITY),
    };
    let holder = database.submit_job(&new, SystemTime::now()).unwrap();
    database
        .take_job(
            holder.id,
            "another",
            SystemTime::now(),
            Duration::from_secs(60),
        )
        .unwrap();
    let refused = gate(&home);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(refused.stdout, "");
    assert!(
        refused.stderr.contains(&holder.id.to_string()),
        "{refused:?}"
    );
    let kept = database.job(&local(&database), holder.id).unwrap().unwrap();
    assert_eq!(kept.state, JobState::Running, "the live holder runs on");
    let imported = home.run(&["knowledge", "import", "--collection", "synthetic"]);
    assert_eq!(
        imported.code,
        Some(0),
        "an import holds a resource of its own: {imported:?}"
    );
}
