//! A new publication attempt resumes from the last step of its predecessor.

use super::support::{Scratch, everything};
use crate::cli::{kernel::Kernel, publish::last_progress};
use maestro_kernel::{
    artifact::Store,
    job::{JobState, NewJob},
    scope::Scope,
};
use maestro_knowledge::index::Progress;
use serde_json::json;
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

#[test]
fn a_retry_uses_the_latest_journaled_step_until_it_records_a_new_one() {
    let scratch = Scratch::new();
    let database = scratch.database();
    let scopes = everything(&database);
    let kernel = Kernel {
        database,
        artifacts: Store::new("unused-artifact-store"),
        scopes,
        config_dir: PathBuf::new(),
    };
    let inputs = json!({"collection": "synthetic", "chunk_set": "set"});
    let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
    let new = NewJob {
        kind: "knowledge.publish",
        inputs: &inputs,
        scope: &scope,
        resource: Some("collection/synthetic/publish"),
    };
    let term = Duration::from_secs(60);
    let first_at = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
    let first = kernel.database.submit_job(&new, first_at).unwrap();
    let mut first_lease = kernel
        .database
        .take_job(first.id, "first", first_at, term)
        .unwrap();
    let first_step = Progress {
        generation: 2,
        indexed: 3,
        chunks: 8,
        average_length: 7.5,
    };
    let last_step = Progress {
        indexed: 6,
        ..first_step.clone()
    };
    kernel
        .database
        .progress(
            &mut first_lease,
            first_at + Duration::from_secs(1),
            term,
            &serde_json::to_value(&first_step).unwrap(),
        )
        .unwrap();
    kernel
        .database
        .progress(
            &mut first_lease,
            first_at + Duration::from_secs(2),
            term,
            &serde_json::to_value(&last_step).unwrap(),
        )
        .unwrap();
    kernel
        .database
        .complete_job(
            &first_lease,
            JobState::Failed,
            &json!({"error": "interrupted"}),
        )
        .unwrap();

    let second_at = first_at + Duration::from_secs(3);
    let second = kernel.database.submit_job(&new, second_at).unwrap();
    assert_eq!(second.attempt, 2);
    assert_eq!(
        last_progress(&kernel, second.id).unwrap(),
        Some(last_step.clone())
    );
    let mut second_lease = kernel
        .database
        .take_job(second.id, "second", second_at, term)
        .unwrap();
    let current_step = Progress {
        indexed: 8,
        ..last_step.clone()
    };
    kernel
        .database
        .progress(
            &mut second_lease,
            second_at + Duration::from_secs(1),
            term,
            &serde_json::to_value(&current_step).unwrap(),
        )
        .unwrap();
    assert_eq!(
        last_progress(&kernel, second.id).unwrap(),
        Some(current_step)
    );
}
