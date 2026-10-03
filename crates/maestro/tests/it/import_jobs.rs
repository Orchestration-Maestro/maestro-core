//! `knowledge import` as a leased job run in the foreground (plan D5, T016):
//! the public synthetic collection imported end to end through the binary,
//! a rerun that returns the same job, and one with `--again` that imports
//! again what the first refused; a resource a live lease holds refused
//! naming its job, and one whose lease expired superseded; a job another
//! process holds followed to its end, and an expired lease taken over, when
//! the rerun starts or while it follows.

use super::support::{
    Home, IMPORT, bind_synthetic_corpus, local, stream, submit_other_import,
    submit_synthetic_import, submit_synthetic_import_with_manifest, synthetic_inputs,
    synthetic_inputs_with_manifest, types,
};
use maestro_kernel::job::JobState;
use serde_json::{Value, json};
use std::{
    fs,
    time::{Duration, SystemTime},
};

/// The report of an import of the whole synthetic collection, 28 documents
/// new.
fn imported_everything() -> Value {
    json!({
        "collection": "synthetic",
        "imported": 28,
        "unchanged": 0,
        "held": 0,
        "refused": 0,
        "refusals": [],
    })
}

/// The report of an import of the test's one-document corpus.
fn imported_one_document() -> Value {
    json!({
        "collection": "synthetic",
        "imported": 1,
        "unchanged": 0,
        "held": 0,
        "refused": 0,
        "refusals": [],
    })
}

/// The files maestro v1 left in the data directory, each with its bytes.
const V1_FILES: [(&str, &str); 3] = [
    ("ledger.sqlite3", "the v1 ledger"),
    ("maestro.sock", "a stale v1 socket"),
    ("material/page.md", "# v1 material\n"),
];

#[test]
fn the_synthetic_collection_imports_end_to_end_through_the_binary() {
    let home = Home::new();
    fs::create_dir(home.data().join("material")).unwrap();
    for (name, bytes) in V1_FILES {
        fs::write(home.data().join(name), bytes).unwrap();
    }
    home.add_synthetic();
    let import = home.run(&["knowledge", "import", "--collection", "synthetic", "--json"]);
    assert_eq!(import.code, Some(0), "{import:?}");
    let document = import.json();
    let id = document["job"].as_str().unwrap().to_owned();
    assert_eq!(
        document,
        json!({
            "schema": "maestro-cli/import/1",
            "job": id,
            "kind": IMPORT,
            "attempt": 1,
            "state": "succeeded",
            "outcome": imported_everything(),
        })
    );
    let database = home.database();
    let job = stream(&database, &format!("job/{id}"));
    assert_eq!(
        types(&job),
        [
            "maestro.job.created.v1",
            "maestro.job.taken.v1",
            "maestro.job.progressed.v1",
            "maestro.job.succeeded.v1",
        ]
    );
    assert_eq!(job[0].data, json!({ "inputs": synthetic_inputs() }));
    let holder = job[1].data["lease"]["holder"].as_str().unwrap();
    let process = holder.strip_prefix("maestro-cli/").unwrap();
    assert!(
        process.parse::<u32>().is_ok(),
        "held by the process: {holder}"
    );
    assert_eq!(
        job[2].data,
        json!({ "imported": 28, "unchanged": 0, "held": 0, "refused": 0 }),
        "one step after the manifest's last line"
    );
    let status = home.run(&["knowledge", "status", "--collection", "synthetic", "--json"]);
    assert_eq!(status.code, Some(0), "{status:?}");
    let status = status.json();
    assert_eq!(status["documents"], 28);
    assert_eq!(status["dispositions"]["undecided"], 28, "{status}");
    assert_eq!(status["generations"], json!([]));
    for (name, bytes) in V1_FILES {
        let kept = fs::read_to_string(home.data().join(name)).unwrap();
        assert_eq!(kept, bytes, "maestro v1's {name} is never touched");
    }
}

#[test]
fn rerunning_an_import_returns_its_job_and_imports_nothing_again() {
    let home = Home::new();
    home.add_synthetic();
    bind_synthetic_corpus(&home, &["en/glossary.md"]);
    let arguments = ["knowledge", "import", "--collection", "synthetic", "--json"];
    let first = home.run(&arguments);
    let second = home.run(&arguments);
    assert_eq!((first.code, second.code), (Some(0), Some(0)), "{second:?}");
    assert_eq!(
        second.json(),
        first.json(),
        "the same job, with the same outcome"
    );
    let database = home.database();
    let completed = stream(&database, "collection/synthetic")
        .into_iter()
        .filter(|event| event.r#type == "maestro.knowledge.import.completed.v1")
        .count();
    assert_eq!(completed, 1, "the rerun imported nothing");
    let id = first.json()["job"].as_str().unwrap().to_owned();
    assert_eq!(
        stream(&database, &format!("job/{id}")).len(),
        4,
        "nor moved the job"
    );
}

#[test]
fn an_import_again_is_a_new_job_that_imports_what_the_first_refused() {
    let home = Home::new();
    home.add_synthetic();
    let root = bind_synthetic_corpus(
        &home,
        &[
            "en/backups/backup-policy.md",
            "en/backups/restoring-a-database.md",
        ],
    );
    let policy = root.join("corpus/en/backups/backup-policy.md");
    let kept = fs::read(&policy).unwrap();
    fs::remove_file(&policy).unwrap();
    let arguments = ["knowledge", "import", "--collection", "synthetic", "--json"];
    let first = home.run(&arguments).json();
    assert_eq!(
        [&first["outcome"]["imported"], &first["outcome"]["refused"]],
        [1, 1]
    );
    fs::write(&policy, kept).unwrap();
    let same = home.run(&arguments).json();
    assert_eq!(same, first, "the same inputs find the same job");
    let again = home.run(&[&arguments[..], &["--again"]].concat());
    assert_eq!(again.code, Some(0), "{again:?}");
    let document = again.json();
    assert_ne!(
        document["job"], first["job"],
        "a job of its own: {document}"
    );
    assert_eq!(document["attempt"], 1, "of a key of its own: {document}");
    let outcome = &document["outcome"];
    assert_eq!(
        [
            &outcome["imported"],
            &outcome["unchanged"],
            &outcome["refused"]
        ],
        [1, 1, 0],
        "{document}"
    );
    let database = home.database();
    let created = &stream(
        &database,
        &format!("job/{}", document["job"].as_str().unwrap()),
    )[0];
    let nonce = created.data["inputs"]["again"].as_str().unwrap();
    assert!(nonce.parse::<ulid::Ulid>().is_ok(), "{nonce}");
    let once_more = home.run(&[&arguments[..], &["--again"]].concat()).json();
    assert_ne!(
        once_more["job"], document["job"],
        "each --again its own job"
    );
    assert_eq!(once_more["outcome"]["unchanged"], 2, "{once_more}");
}

#[test]
fn an_import_whose_resource_a_live_lease_holds_is_refused_naming_its_job() {
    let home = Home::new();
    home.add_synthetic();
    let database = home.database();
    let holder = submit_other_import(&database);
    database
        .take_job(
            holder.id,
            "another",
            SystemTime::now(),
            Duration::from_secs(60),
        )
        .unwrap();
    let refused = home.run(&["knowledge", "import", "--collection", "synthetic", "--json"]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(refused.stdout, "");
    assert!(
        refused.stderr.contains(&holder.id.to_string()),
        "{refused:?}"
    );
    let kept = database.job(&local(&database), holder.id).unwrap().unwrap();
    assert_eq!(kept.state, JobState::Running, "the live holder runs on");
}

#[test]
fn an_import_supersedes_the_job_of_other_inputs_whose_lease_expired() {
    let home = Home::new();
    home.add_synthetic();
    let root = bind_synthetic_corpus(&home, &["en/glossary.md"]);
    let manifest = root.join("corpus/maestro-corpus.jsonl");
    let database = home.database();
    let stale = submit_other_import(&database);
    let long_ago = SystemTime::now() - Duration::from_secs(120);
    database
        .take_job(stale.id, "crashed", long_ago, Duration::from_secs(60))
        .unwrap();
    let import = home.run(&["knowledge", "import", "--collection", "synthetic", "--json"]);
    assert_eq!(import.code, Some(0), "{import:?}");
    let document = import.json();
    assert_ne!(document["job"], stale.id.to_string(), "a job of its own");
    assert_eq!(
        (&document["state"], &document["outcome"]),
        (&json!("succeeded"), &imported_one_document())
    );
    let superseded = database.job(&local(&database), stale.id).unwrap().unwrap();
    assert_eq!(
        (superseded.state, superseded.outcome),
        (
            JobState::Cancelled,
            Some(json!({
                "superseded_by": { "inputs": synthetic_inputs_with_manifest(&manifest) }
            }))
        )
    );
    assert_eq!(
        types(&stream(&database, &format!("job/{}", stale.id))),
        [
            "maestro.job.created.v1",
            "maestro.job.taken.v1",
            "maestro.job.taken_over.v1",
            "maestro.job.cancelled.v1",
        ]
    );
}

#[test]
fn a_rerun_follows_the_job_another_process_holds_to_its_end() {
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
    let mut running = home.start(&["knowledge", "import", "--collection", "synthetic"]);
    assert_eq!(running.line(), format!("job {}", job.id));
    running.line_with("maestro.job.taken.v1");
    let outcome = json!({ "collection": "synthetic", "imported": 0 });
    database
        .complete_job(&lease, JobState::Succeeded, &outcome)
        .unwrap();
    let ended = running.finish();
    assert_eq!(ended.code, Some(0), "{ended:?}");
    assert!(
        ended.stdout.ends_with(&format!("succeeded {outcome}\n")),
        "{ended:?}"
    );
    let imported = stream(&database, "collection/synthetic")
        .into_iter()
        .any(|event| event.r#type == "maestro.knowledge.import.completed.v1");
    assert!(!imported, "the follower imported nothing");
}

#[test]
fn an_expired_lease_is_taken_over_and_the_import_run_again() {
    let home = Home::new();
    home.add_synthetic();
    let root = bind_synthetic_corpus(&home, &["en/glossary.md"]);
    let manifest = root.join("corpus/maestro-corpus.jsonl");
    let database = home.database();
    let job = submit_synthetic_import_with_manifest(&database, &manifest);
    let long_ago = SystemTime::now() - Duration::from_secs(120);
    database
        .take_job(job.id, "crashed", long_ago, Duration::from_secs(60))
        .unwrap();
    let import = home.run(&["knowledge", "import", "--collection", "synthetic", "--json"]);
    assert_eq!(import.code, Some(0), "{import:?}");
    let document = import.json();
    assert_eq!(document["job"], job.id.to_string());
    assert_eq!(document["attempt"], 1);
    assert_eq!(document["outcome"], imported_one_document());
    let events = stream(&database, &format!("job/{}", job.id));
    assert!(
        types(&events).contains(&"maestro.job.taken_over.v1"),
        "{:?}",
        types(&events)
    );
}

#[test]
fn a_rerun_takes_over_a_lease_that_expires_while_it_follows() {
    let home = Home::new();
    home.add_synthetic();
    let root = bind_synthetic_corpus(&home, &["en/glossary.md"]);
    let manifest = root.join("corpus/maestro-corpus.jsonl");
    let database = home.database();
    let job = submit_synthetic_import_with_manifest(&database, &manifest);
    // Live for two more seconds: the rerun follows it, then takes it over.
    let taken = SystemTime::now() - Duration::from_secs(58);
    database
        .take_job(job.id, "crashed", taken, Duration::from_secs(60))
        .unwrap();
    let mut running = home.start(&["knowledge", "import", "--collection", "synthetic"]);
    assert_eq!(running.line(), format!("job {}", job.id));
    running.line_with("maestro.job.taken.v1");
    let ended = running.finish();
    assert_eq!(ended.code, Some(0), "{ended:?}");
    assert!(
        ended.stdout.ends_with(concat!(
            "succeeded {\"collection\":\"synthetic\",\"held\":0,\"imported\":1,",
            "\"refusals\":[],\"refused\":0,\"unchanged\":0}\n",
        )),
        "{ended:?}"
    );
    assert_eq!(
        types(&stream(&database, &format!("job/{}", job.id))),
        [
            "maestro.job.created.v1",
            "maestro.job.taken.v1",
            "maestro.job.taken_over.v1",
            "maestro.job.progressed.v1",
            "maestro.job.succeeded.v1",
        ]
    );
}

#[test]
fn foreground_step_keeps_pre_cedar_display_bytes() {
    let home = Home::new();
    home.add_synthetic();
    bind_synthetic_corpus(&home, &["en/glossary.md"]);
    let output = home.run(&["knowledge", "import", "--collection", "synthetic"]);
    assert_eq!(output.code, Some(0), "{output:?}");
    let step = output
        .stdout
        .lines()
        .find(|line| line.starts_with("step "))
        .unwrap();
    assert_eq!(
        step,
        r#"step {"held":0,"imported":1,"refused":0,"unchanged":0}"#
    );
}
