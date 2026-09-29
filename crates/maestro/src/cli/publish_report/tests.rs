//! Pure report formatting and database-to-Qdrant reconciliation checks.

use super::*;
use crate::{cli::output::Output, failure::Failure, kernel::Kernel};
use maestro_kernel::{
    artifact::{Digest, Store},
    chunk_set::NewChunkSet,
    document::Collection,
    gateway::{CardFields, Limits, ModelCard, Role, RouterEntry},
    generation::{Generation, GenerationState, NewGeneration},
    job::{Job, JobState},
    retrieval::IDENTIFIER_PROFILE,
    scope::{Right, Scope},
    store::Database,
};
use maestro_knowledge::lexical;
use maestro_test_scratch::scratch_directory;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    net::TcpListener,
    num::{NonZeroU32, NonZeroUsize},
    path::PathBuf,
    process::ExitCode,
};
use ulid::Ulid;

const COLLECTION: &str = "synthetic";
const CHUNK_SET: &str = "set";

struct Fixture {
    kernel: Kernel,
    card: ModelCard,
    job: Job,
    inputs: Value,
    _scratch: Scratch,
}

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        Self(scratch_directory().unwrap())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn fixture(generation: Option<(&str, &str, &str)>, search: Option<(&str, bool)>) -> Fixture {
    let scratch = Scratch::new();
    let database = Database::open_in(&scratch.0).unwrap();
    let artifacts = Store::new(scratch.0.join("artifacts"));
    let card = ModelCard::record(
        &artifacts,
        &CardFields {
            role: Role::Embedder,
            router_entry: RouterEntry::parse("synthetic-embed").unwrap(),
            file_digest: Digest::of(b"synthetic model"),
            template_digest: None,
            server_build: "synthetic".to_owned(),
            dimensions: NonZeroUsize::new(8),
            limits: Limits {
                context_tokens: NonZeroU32::new(512).unwrap(),
                output_tokens: None,
            },
            suite_results: Vec::new(),
        },
    )
    .unwrap();
    database
        .record_collection(&Collection {
            id: COLLECTION.to_owned(),
            title: "Synthetic".to_owned(),
            visibility: "public".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    let workspace: Scope = "workspace/default".parse().unwrap();
    database
        .grant("report-test", &workspace, Right::Read, "test")
        .unwrap();
    let scopes = database.visible("report-test").unwrap();
    complete_set(&database, CHUNK_SET);
    let kernel = Kernel {
        database: database.into(),
        artifacts,
        scopes,
        config_dir: PathBuf::new(),
        test_refresh_hook: None,
    };
    let current = generation.map(|(chunk_set, embedding, sparse)| {
        if chunk_set != CHUNK_SET {
            complete_set(&kernel.database, chunk_set);
        }
        let generation = kernel
            .database
            .create_generation(&NewGeneration {
                collection_id: COLLECTION.to_owned(),
                chunk_set_id: chunk_set.to_owned(),
                embedding_profile: embedding.to_owned(),
                sparse_profile: sparse.to_owned(),
            })
            .unwrap();
        if let Some((profile, ready)) = search {
            kernel
                .database
                .begin_generation_search(&kernel.scopes, generation.id, profile)
                .unwrap();
            if ready {
                kernel
                    .database
                    .complete_generation_search(&kernel.scopes, generation.id)
                    .unwrap();
            }
        }
        kernel.database.verify_generation(generation.id, 0).unwrap();
        kernel.database.publish_generation(generation.id).unwrap();
        generation.id
    });
    let card_digest = card.digest().as_str().to_owned();
    let inputs = json!({
        "card": card_digest,
        "chunk_set": CHUNK_SET,
        "collection": COLLECTION,
        "identifier_profile": IDENTIFIER_PROFILE,
        "sparse_profile": lexical::PROFILE,
    });
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    let job = Job {
        id: Ulid::generate(),
        kind: "knowledge.publish".to_owned(),
        idempotency_key: Digest::of(b"report job"),
        attempt: 1,
        scope,
        resource: Some("collection/synthetic/publish".to_owned()),
        state: JobState::Succeeded,
        lease: None,
        outcome: Some(json!({"generation": current.unwrap_or(1)})),
    };
    Fixture {
        kernel,
        card,
        job,
        inputs,
        _scratch: scratch,
    }
}

fn complete_set(database: &Database, id: &str) {
    database
        .begin_chunk_set(&NewChunkSet {
            id,
            collection_id: COLLECTION,
            chunk_profile: "synthetic/1",
            counter_contract_id: "synthetic-counter/1",
        })
        .unwrap();
    let manifest = database.put(b"{}", "application/json").unwrap();
    database.complete_chunk_set(id, &manifest).unwrap();
}

fn embedding(card: &ModelCard) -> String {
    format!("dense/1:sha256:{}", card.digest().as_str())
}

fn render(fixture: &Fixture, qdrant_url: &str) -> Result<ExitCode, Failure> {
    super::render(
        &fixture.kernel,
        Output::new(true),
        ReportRequest {
            job: &fixture.job,
            inputs: &fixture.inputs,
            card: &fixture.card,
            qdrant_url,
            resuming: false,
        },
    )
}

fn unreachable_qdrant() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{address}")
}

fn text_document<'a>(
    job: &'a Job,
    outcome: &'a Value,
    current_generation: Option<i64>,
    projection_ready: bool,
    resuming: bool,
) -> PublishDocument<'a> {
    PublishDocument {
        schema: SCHEMA,
        job: job.id.to_string(),
        kind: &job.kind,
        attempt: job.attempt,
        state: job.state.to_string(),
        outcome,
        historical_generation: outcome.get("generation").and_then(Value::as_i64),
        current_generation,
        projection_ready,
        resuming,
    }
}

fn job(state: JobState, outcome: Value) -> Job {
    Job {
        id: Ulid::generate(),
        kind: "knowledge.publish".to_owned(),
        idempotency_key: Digest::of(b"text report job"),
        attempt: 2,
        scope: "workspace/default/collection/synthetic".parse().unwrap(),
        resource: None,
        state,
        lease: None,
        outcome: Some(outcome),
    }
}

#[test]
fn text_report_names_the_recovery_command_only_for_a_succeeded_missing_projection() {
    let scratch = Scratch::new();
    let store = Store::new(scratch.0.join("artifacts"));
    let card = ModelCard::record(
        &store,
        &CardFields {
            role: Role::Embedder,
            router_entry: RouterEntry::parse("synthetic-embed").unwrap(),
            file_digest: Digest::of(b"synthetic model"),
            template_digest: None,
            server_build: "synthetic".to_owned(),
            dimensions: NonZeroUsize::new(8),
            limits: Limits {
                context_tokens: NonZeroU32::new(512).unwrap(),
                output_tokens: None,
            },
            suite_results: Vec::new(),
        },
    )
    .unwrap();
    let outcome = json!({"generation": 4});
    let succeeded = job(JobState::Succeeded, outcome.clone());
    let next = recovery_command(COLLECTION, CHUNK_SET, &card);
    assert_eq!(
        next,
        format!(
            concat!(
                "maestro knowledge publish --collection {} ",
                "--chunk-set {} --card {} --again"
            ),
            COLLECTION,
            CHUNK_SET,
            card.digest().as_str()
        )
    );
    let missing = text_document(&succeeded, &outcome, Some(5), false, true);
    let line = publish_line(
        &succeeded,
        &missing,
        &json!({
            "collection": COLLECTION,
            "chunk_set": CHUNK_SET,
        }),
        &card,
    );
    assert!(line.contains("historical generation 4, current generation 5, projection not ready"));
    assert!(line.contains("resumed unfinished publication"));
    assert!(line.contains(&format!("next: run `{next}`")));

    let ready = text_document(&succeeded, &outcome, Some(5), true, false);
    let line = publish_line(&succeeded, &ready, &json!({}), &card);
    assert!(line.contains("projection ready"));
    assert!(!line.contains("next: run"));
    assert!(!line.contains("outcome"));

    let failed = job(JobState::Failed, json!({"error": "unavailable"}));
    let failed_outcome = failed.outcome.as_ref().unwrap();
    let document = text_document(&failed, failed_outcome, None, false, false);
    let line = publish_line(&failed, &document, &json!({}), &card);
    assert!(line.contains("outcome {\"error\":\"unavailable\"}"));
    assert!(!line.contains("next: run"));
}

#[test]
fn projection_readiness_requires_the_collection_and_its_matching_alias() {
    assert!(render::projection_is_ready(
        true,
        Some("collection"),
        "collection"
    ));
    assert!(!render::projection_is_ready(
        false,
        Some("collection"),
        "collection"
    ));
    assert!(!render::projection_is_ready(true, None, "collection"));
    assert!(!render::projection_is_ready(
        true,
        Some("other"),
        "collection"
    ));
    assert_eq!(
        render::report_exit_code(JobState::Succeeded, true),
        ExitCode::SUCCESS
    );
    assert_eq!(
        render::report_exit_code(JobState::Succeeded, false),
        ExitCode::from(1)
    );
    assert_eq!(
        render::report_exit_code(JobState::Failed, true),
        ExitCode::from(1)
    );
}

#[test]
fn generation_labels_and_qdrant_names_are_stable() {
    let generation = Generation {
        id: 8,
        collection_id: COLLECTION.to_owned(),
        chunk_set_id: CHUNK_SET.to_owned(),
        embedding_profile: "dense/test".to_owned(),
        sparse_profile: lexical::PROFILE.to_owned(),
        state: GenerationState::Published,
        point_count: Some(3),
        published_at: Some("2026-01-01T00:00:00Z".to_owned()),
    };
    assert_eq!(generation_label(None), "none");
    assert_eq!(generation_label(Some(8)), "8");
    assert_eq!(generation_collection(&generation), "maestro-synthetic-g8");
    assert_eq!(generation_alias(&generation), "maestro-synthetic");
}

#[test]
fn report_skips_qdrant_when_no_current_generation_or_the_frozen_tuple_differs() {
    let none = fixture(None, None);
    assert_eq!(
        render(&none, &unreachable_qdrant()).unwrap(),
        ExitCode::from(1)
    );
    let card = none.card.digest().as_str().to_owned();
    let cases = [
        (
            "other",
            format!("dense/1:sha256:{card}"),
            lexical::PROFILE.to_owned(),
        ),
        (
            CHUNK_SET,
            "wrong-card".to_owned(),
            lexical::PROFILE.to_owned(),
        ),
        (
            CHUNK_SET,
            format!("dense/1:sha256:{card}"),
            "wrong-sparse".to_owned(),
        ),
    ];
    for (chunk_set, embedding, sparse) in cases {
        let mismatched = fixture(
            Some((chunk_set, &embedding, &sparse)),
            Some((IDENTIFIER_PROFILE, true)),
        );
        assert_eq!(
            render(&mismatched, &unreachable_qdrant()).unwrap(),
            ExitCode::from(1)
        );
    }
}

#[test]
fn report_skips_qdrant_when_the_search_marker_is_missing_unready_or_for_another_profile() {
    let card_digest = {
        let fixture = fixture(None, None);
        fixture.card.digest().as_str().to_owned()
    };
    let matching_embedding = format!("dense/1:sha256:{card_digest}");
    for search in [
        None,
        Some((IDENTIFIER_PROFILE, false)),
        Some(("identifiers/other", true)),
    ] {
        let not_ready = fixture(
            Some((CHUNK_SET, &matching_embedding, lexical::PROFILE)),
            search,
        );
        assert_eq!(
            render(&not_ready, &unreachable_qdrant()).unwrap(),
            ExitCode::from(1)
        );
    }
}

#[test]
fn report_propagates_qdrant_errors_after_a_matching_ready_search_projection() {
    let no_card = fixture(None, None);
    let matching_embedding = embedding(&no_card.card);
    let ready = fixture(
        Some((CHUNK_SET, &matching_embedding, lexical::PROFILE)),
        Some((IDENTIFIER_PROFILE, true)),
    );
    assert!(render(&ready, &unreachable_qdrant()).is_err());
}
