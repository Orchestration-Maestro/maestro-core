//! The import rate, measured on demand for T019's report: 2,000 small
//! generated documents imported, then imported again, which reads, checks
//! and canonicalizes each but writes nothing; then the two units a write
//! costs, 2,000 commits of one event and 2,000 artifacts stored.

use super::support::{Scratch, everything, import, line, markdown};
use maestro_kernel::journal::NewEvent;
use serde_json::Value;
use std::time::{Duration, Instant};

/// How many documents the measurement imports.
const DOCUMENTS: u32 = 2_000;

/// The rate of `count` items in `elapsed`, per second.
fn rate(count: u32, elapsed: Duration) -> f64 {
    f64::from(count) / elapsed.as_secs_f64()
}

#[test]
#[ignore = "a measurement, run in a release build: cargo test --release -p maestro-knowledge \
            --test it import_rate -- --ignored --nocapture"]
fn measures_the_import_rate_of_two_thousand_small_documents() {
    let scratch = Scratch::new();
    let lines: Vec<Value> = (0..DOCUMENTS)
        .map(|index| {
            let bytes = markdown(&format!("topic {index}"));
            let path = format!("pages/{index}.md");
            scratch.put(&path, &bytes);
            line(&path, &bytes, &format!("https://example.org/pages/{index}"))
        })
        .collect();
    scratch.manifest(&lines);
    let database = scratch.database();
    let started = Instant::now();
    let first = import(&scratch, &database, "garden");
    let importing = started.elapsed();
    assert_eq!(first.imported, u64::from(DOCUMENTS));
    let started = Instant::now();
    let second = import(&scratch, &database, "garden");
    let reading = started.elapsed();
    assert_eq!(second.unchanged, u64::from(DOCUMENTS));
    let scopes = everything(&database);
    assert!(!scopes.is_empty());
    let started = Instant::now();
    for index in 0..DOCUMENTS {
        let subject = format!("probe/{index}");
        database
            .record(&NewEvent {
                stream: "probe",
                r#type: "maestro.test.probe.v1",
                subject: &subject,
                scope: "workspace/default",
                data: &Value::Null,
            })
            .unwrap();
    }
    let committing = started.elapsed();
    let started = Instant::now();
    for index in 0..DOCUMENTS {
        let bytes = format!("probe {index}\n");
        database.put(bytes.as_bytes(), "text/plain").unwrap();
    }
    let storing = started.elapsed();
    println!(
        "import: {importing:?} for {DOCUMENTS} documents, {:.0} per second",
        rate(DOCUMENTS, importing)
    );
    println!(
        "re-import, nothing written: {reading:?}, {:.0} per second",
        rate(DOCUMENTS, reading)
    );
    println!(
        "{DOCUMENTS} commits of one event: {committing:?}, {:.0} per second",
        rate(DOCUMENTS, committing)
    );
    println!(
        "{DOCUMENTS} artifacts stored: {storing:?}, {:.0} per second",
        rate(DOCUMENTS, storing)
    );
}
