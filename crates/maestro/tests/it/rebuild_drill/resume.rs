//! A backed-up expired replacement attempt resumes against a new Qdrant target.

use crate::support::Home;
use maestro_kernel::{
    generation::{GenerationState, NewGeneration},
    job::{self, JobState, NewJob},
    journal::Filter,
    retrieval::IDENTIFIER_PROFILE,
    scope::{LOCAL, Scope},
};
use maestro_knowledge::{index::Progress, lexical};
use serde_json::{Value, json};
use std::time::{Duration, SystemTime};
use ulid::Ulid;

const COLLECTION: &str = "synthetic";
const RESOURCE: &str = "collection/synthetic/publish";
const BATCH_SIZE: u64 = 64;

pub(super) struct Interrupted {
    pub(super) job: Ulid,
    pub(super) target: i64,
    pub(super) progress_sequence: u64,
}

pub(super) fn create(
    home: &Home,
    set_id: &str,
    card: &str,
    old_generation: i64,
    baseline_progress: &Value,
) -> Interrupted {
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    let previous = database
        .generation(&scopes, old_generation)
        .unwrap()
        .unwrap();
    let target = database
        .create_generation(&NewGeneration {
            collection_id: COLLECTION.to_owned(),
            chunk_set_id: set_id.to_owned(),
            embedding_profile: previous.embedding_profile,
            sparse_profile: lexical::PROFILE.to_owned(),
        })
        .unwrap();
    assert_eq!(target.state, GenerationState::Building);
    let nonce = Ulid::generate().to_string();
    let inputs = json!({
        "again": nonce,
        "card": card,
        "chunk_set": set_id,
        "collection": COLLECTION,
        "expected_published": old_generation,
        "generation_watermark": old_generation,
        "identifier_profile": IDENTIFIER_PROFILE,
        "sparse_profile": lexical::PROFILE,
    });
    let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
    let new = NewJob {
        kind: "knowledge.publish",
        inputs: &inputs,
        scope: &scope,
        resource: Some(RESOURCE),
    };
    let started = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000);
    let job = database.submit_job(&new, started).unwrap();
    let term = Duration::from_secs(2);
    let mut lease = database
        .take_job(job.id, "t033b-interrupted", started, term)
        .unwrap();
    let original: Progress = serde_json::from_value(baseline_progress.clone()).unwrap();
    assert!(original.chunks > BATCH_SIZE);
    let progress = Progress {
        generation: target.id,
        indexed: BATCH_SIZE,
        chunks: original.chunks,
        average_length: original.average_length,
    };
    let event = database
        .progress(
            &mut lease,
            started + Duration::from_secs(1),
            term,
            &serde_json::to_value(progress).unwrap(),
        )
        .unwrap();
    assert_eq!(
        database.job(&scopes, job.id).unwrap().unwrap().state,
        JobState::Running
    );
    Interrupted {
        job: job.id,
        target: target.id,
        progress_sequence: event.sequence,
    }
}

pub(super) fn assert_restored(home: &Home, interrupted: &Interrupted) {
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    let job = database.job(&scopes, interrupted.job).unwrap().unwrap();
    assert_eq!(job.state, JobState::Running);
    assert_eq!(job.attempt, 1);
    assert_eq!(job.lease.as_ref().unwrap().holder, "t033b-interrupted");
    let progress = database
        .last_progress(&scopes, interrupted.job)
        .unwrap()
        .unwrap();
    assert_eq!(progress.sequence, interrupted.progress_sequence);
    assert_eq!(progress.data["generation"], interrupted.target);
    assert_eq!(progress.data["indexed"], BATCH_SIZE);
    assert_eq!(
        database
            .generation(&scopes, interrupted.target)
            .unwrap()
            .unwrap()
            .state,
        GenerationState::Building
    );
}

pub(super) fn takeover_number(home: &Home, interrupted: &Interrupted) -> u64 {
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    let stream = job::stream(interrupted.job);
    let events = database
        .events(
            &scopes,
            &Filter {
                stream: &stream,
                after: interrupted.progress_sequence,
                r#type: Some(job::TAKEN_OVER),
            },
        )
        .unwrap();
    assert_eq!(
        events.len(),
        1,
        "expired publication job was not taken over"
    );
    events[0].data["lease"]["number"].as_u64().unwrap()
}

pub(super) fn expected_replayed_indices(chunks: u64) -> Vec<u64> {
    let mut indexed = 0;
    let mut indices = Vec::new();
    while indexed < chunks {
        indexed = indexed.saturating_add(BATCH_SIZE).min(chunks);
        indices.push(indexed);
    }
    indices
}

pub(super) fn replayed_indices(home: &Home, interrupted: &Interrupted) -> Vec<u64> {
    let database = home.database();
    let scopes = database.visible(LOCAL).unwrap();
    let stream = job::stream(interrupted.job);
    database
        .events(
            &scopes,
            &Filter {
                stream: &stream,
                after: interrupted.progress_sequence,
                r#type: Some(job::PROGRESSED),
            },
        )
        .unwrap()
        .into_iter()
        .map(|event| event.data["indexed"].as_u64().unwrap())
        .collect()
}
