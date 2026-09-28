//! Selection of active and failed explicit replacement jobs.

use crate::{
    cli::{
        args::PublishArguments,
        output::Output,
        publish::{self, ResumeCandidate},
        tests::support::{Scratch, everything},
    },
    failure::Failure,
    kernel::Kernel,
};
use maestro_kernel::{
    artifact::Digest,
    chunk_set::NewChunkSet,
    document::Collection,
    gateway::{CardFields, Limits, ModelCard, Role, RouterEntry},
    generation::{Generation, GenerationState, NewGeneration},
    job::{Job, JobState, NewJob},
    journal::{NewEvent, stream},
    retrieval::IDENTIFIER_PROFILE,
    scope::Scope,
};
use maestro_knowledge::{
    index::{Progress, RebuildGuard},
    lexical,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    num::{NonZeroU32, NonZeroUsize},
    path::PathBuf,
    slice,
    time::{Duration, SystemTime},
};
use ulid::Ulid;

const COLLECTION: &str = "synthetic";
const CHUNK_SET: &str = "set";
const RESOURCE: &str = "collection/synthetic/publish";
const KIND: &str = "knowledge.publish";

#[path = "selection/jobs.rs"]
mod jobs;

struct Fixture {
    /// The scoped kernel under test.
    kernel: Kernel,
    /// Its synthetic embedder card.
    card: ModelCard,
    /// The original published generation.
    original: i64,
    /// Kept alive until its kernel and artifacts are dropped.
    _scratch: Scratch,
}

fn fixture() -> Fixture {
    let scratch = Scratch::new();
    let database = scratch.database();
    let scopes = everything(&database);
    let artifacts = scratch.artifacts();
    let fields = CardFields {
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
    };
    let card = ModelCard::record(&artifacts, &fields).unwrap();
    let card_json = artifacts.get(card.digest()).unwrap();
    database.put(&card_json, "application/json").unwrap();
    let kernel = Kernel {
        database: database.into(),
        artifacts,
        scopes,
        config_dir: PathBuf::new(),
        test_refresh_hook: None,
    };
    kernel
        .database
        .record_collection(&Collection {
            id: COLLECTION.to_owned(),
            title: "Synthetic".to_owned(),
            visibility: "public".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    let manifest = kernel
        .database
        .put(b"chunk set manifest", "application/json")
        .unwrap();
    kernel
        .database
        .begin_chunk_set(&NewChunkSet {
            id: CHUNK_SET,
            collection_id: COLLECTION,
            chunk_profile: "synthetic/1",
            counter_contract_id: "synthetic-counter/1",
        })
        .unwrap();
    kernel
        .database
        .complete_chunk_set(CHUNK_SET, &manifest)
        .unwrap();
    let original = generation_for(&kernel, &card, GenerationState::Published).id;
    Fixture {
        kernel,
        card,
        original,
        _scratch: scratch,
    }
}

fn generation(fixture: &Fixture, state: GenerationState) -> Generation {
    generation_for(&fixture.kernel, &fixture.card, state)
}

fn generation_for(kernel: &Kernel, card: &ModelCard, state: GenerationState) -> Generation {
    let generation = kernel
        .database
        .create_generation(&NewGeneration {
            collection_id: COLLECTION.to_owned(),
            chunk_set_id: CHUNK_SET.to_owned(),
            embedding_profile: format!("dense/1:sha256:{}", card.digest().as_str()),
            sparse_profile: lexical::PROFILE.to_owned(),
        })
        .unwrap();
    if matches!(
        state,
        GenerationState::Verified | GenerationState::Published
    ) {
        kernel.database.verify_generation(generation.id, 0).unwrap();
    }
    if state == GenerationState::Published {
        kernel.database.publish_generation(generation.id).unwrap();
    }
    generation
}

fn original_guard(fixture: &Fixture) -> RebuildGuard {
    RebuildGuard {
        expected_published: Some(fixture.original),
        generation_watermark: fixture.original,
    }
}

fn attempt(
    fixture: &Fixture,
    guard: RebuildGuard,
    target: i64,
    state: JobState,
    seconds: u64,
) -> (Job, Value, Progress) {
    let inputs = publish::recovery_inputs(
        fixture.card.digest().as_str(),
        CHUNK_SET,
        COLLECTION,
        guard,
        Ulid::generate(),
    );
    let scope: Scope = "workspace/default/collection/synthetic".parse().unwrap();
    let new = NewJob {
        kind: KIND,
        inputs: &inputs,
        scope: &scope,
        resource: Some(RESOURCE),
    };
    let now = SystemTime::UNIX_EPOCH + Duration::from_secs(10_000 + seconds);
    let job = fixture.kernel.database.submit_job(&new, now).unwrap();
    let term = Duration::from_secs(60);
    let mut lease = fixture
        .kernel
        .database
        .take_job(job.id, "publish-again-test", now, term)
        .unwrap();
    let progress = Progress {
        generation: target,
        indexed: 0,
        chunks: 1,
        average_length: 0.0,
    };
    fixture
        .kernel
        .database
        .progress(
            &mut lease,
            now + Duration::from_secs(1),
            term,
            &serde_json::to_value(&progress).unwrap(),
        )
        .unwrap();
    if state == JobState::Failed {
        fixture
            .kernel
            .database
            .complete_job(&lease, state, &json!({"error": "switch failed"}))
            .unwrap();
    }
    (job, inputs, progress)
}

fn arguments(fixture: &Fixture, again: bool) -> PublishArguments {
    PublishArguments {
        collection: COLLECTION.to_owned(),
        card: fixture.card.digest().as_str().to_owned(),
        chunk_set: Some(CHUNK_SET.to_owned()),
        chunk_profile: None,
        again,
    }
}

fn record_declaration(fixture: &Fixture, collection: &str) {
    let mut declaration: Value = serde_json::from_str(include_str!(
        "../../../../../../tests/fixtures/synthetic/collection.json"
    ))
    .unwrap();
    declaration["id"] = json!(collection);
    let declaration = serde_json::to_vec(&declaration).unwrap();
    let digest = fixture
        .kernel
        .database
        .put(&declaration, "application/json")
        .unwrap();
    let stream = stream(collection);
    let scope: Scope = format!("workspace/default/collection/{collection}")
        .parse()
        .unwrap();
    let data = json!({
        "collection": collection,
        "declaration": digest.as_str(),
        "path": format!("/tmp/{collection}/collection.json"),
    });
    fixture
        .kernel
        .database
        .record(&NewEvent {
            stream: &stream,
            r#type: "maestro.knowledge.collection.added.v1",
            subject: &stream,
            scope: scope.as_str(),
            data: &data,
        })
        .unwrap();
}

fn selected(
    fixture: &Fixture,
    again: bool,
) -> Result<(Value, Option<RebuildGuard>, bool), Failure> {
    publish::selected_inputs(
        &fixture.kernel,
        RESOURCE,
        &arguments(fixture, again),
        &fixture.card,
        CHUNK_SET,
    )
}

fn candidate(
    state: JobState,
    expected: Option<i64>,
    progress_generation: Option<i64>,
    target_state: Option<GenerationState>,
) -> ResumeCandidate {
    let guard = RebuildGuard {
        expected_published: expected,
        generation_watermark: 4,
    };
    let inputs = publish::recovery_inputs("card", "set", "synthetic", guard, Ulid::generate());
    ResumeCandidate {
        id: Ulid::generate(),
        state,
        recovery: guard,
        progress_generation,
        target_state,
        inputs,
    }
}

#[test]
fn recovery_inputs_freeze_the_tuple_nonce_expected_pointer_and_watermark() {
    let guard = RebuildGuard {
        expected_published: Some(7),
        generation_watermark: 9,
    };
    let nonce = Ulid::generate();
    let inputs = publish::recovery_inputs("card", "set", "synthetic", guard, nonce);
    assert_eq!(
        inputs,
        json!({
            "card": "card",
            "chunk_set": "set",
            "collection": "synthetic",
            "identifier_profile": IDENTIFIER_PROFILE,
            "sparse_profile": lexical::PROFILE,
            "again": nonce.to_string(),
            "expected_published": 7,
            "generation_watermark": 9,
        })
    );
}

#[test]
fn publish_preflight_refuses_missing_unknown_and_incomplete_chunk_sets() {
    let fixture = fixture();
    record_declaration(&fixture, COLLECTION);

    let mut no_recovery_set = arguments(&fixture, true);
    no_recovery_set.chunk_set = None;
    assert!(matches!(
        publish::run(&fixture.kernel, Output::new(true), &no_recovery_set),
        Err(Failure::Refused(message)) if message.contains("--again requires --chunk-set")
    ));

    let mut unknown = arguments(&fixture, false);
    unknown.chunk_set = Some("missing".to_owned());
    let unknown_error = publish::run(&fixture.kernel, Output::new(true), &unknown);
    let refused_unknown = matches!(
        &unknown_error,
        Err(Failure::Refused(message)) if message.contains("no chunk set missing")
    );
    assert!(refused_unknown, "{unknown_error:?}");

    fixture
        .kernel
        .database
        .begin_chunk_set(&NewChunkSet {
            id: "building-set",
            collection_id: COLLECTION,
            chunk_profile: "synthetic/1",
            counter_contract_id: "synthetic-counter/1",
        })
        .unwrap();
    let mut building = arguments(&fixture, false);
    building.chunk_set = Some("building-set".to_owned());
    assert!(matches!(
        publish::run(&fixture.kernel, Output::new(true), &building),
        Err(Failure::Refused(message)) if message.contains("building-set is building")
    ));

    fixture
        .kernel
        .database
        .record_collection(&Collection {
            id: "empty".to_owned(),
            title: "Empty".to_owned(),
            visibility: "private".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    record_declaration(&fixture, "empty");
    let empty = PublishArguments {
        collection: "empty".to_owned(),
        card: fixture.card.digest().as_str().to_owned(),
        chunk_set: None,
        chunk_profile: None,
        again: false,
    };
    assert!(matches!(
        publish::run(&fixture.kernel, Output::new(true), &empty),
        Err(Failure::Refused(message))
            if message == "collection empty has no complete chunk set of mapped-structural-chunks/2"
    ));
}

#[test]
fn recovery_reads_absent_and_present_expected_pointers_and_requires_a_nonnegative_watermark() {
    let absent = publish::recovery_inputs(
        "card",
        "set",
        "synthetic",
        RebuildGuard {
            expected_published: None,
            generation_watermark: 0,
        },
        Ulid::generate(),
    );
    assert_eq!(
        publish::recovery(&absent).unwrap(),
        RebuildGuard {
            expected_published: None,
            generation_watermark: 0,
        }
    );
    let present = publish::recovery_inputs(
        "card",
        "set",
        "synthetic",
        RebuildGuard {
            expected_published: Some(2),
            generation_watermark: 8,
        },
        Ulid::generate(),
    );
    assert_eq!(
        publish::recovery(&present).unwrap(),
        RebuildGuard {
            expected_published: Some(2),
            generation_watermark: 8,
        }
    );
    let mut negative = present.clone();
    negative["generation_watermark"] = json!(-1);
    assert!(publish::recovery(&negative).is_err());
    let mut negative_expected = present.clone();
    negative_expected["expected_published"] = json!(-1);
    assert!(publish::recovery(&negative_expected).is_err());
    let mut zero_expected = present.clone();
    zero_expected["expected_published"] = json!(0);
    assert!(publish::recovery(&zero_expected).is_err());
    let mut missing = present.clone();
    missing
        .as_object_mut()
        .unwrap()
        .remove("expected_published");
    assert!(publish::recovery(&missing).is_err());
}

#[test]
fn no_matching_job_starts_a_new_explicit_publication() {
    assert!(publish::select_resume(&[], Some(2)).unwrap().is_none());
}

#[test]
fn one_active_job_with_its_frozen_pointer_is_reused() {
    let pending = candidate(JobState::Running, Some(2), None, None);
    let expected_inputs = pending.inputs.clone();
    let selected = publish::select_resume(slice::from_ref(&pending), Some(2))
        .unwrap()
        .unwrap();
    assert_eq!(selected, expected_inputs);
}

#[test]
fn an_active_job_reconciles_its_own_published_target_after_a_crash() {
    let pending = candidate(
        JobState::Running,
        Some(1),
        Some(2),
        Some(GenerationState::Published),
    );
    let expected_inputs = pending.inputs.clone();
    let selected = publish::select_resume(slice::from_ref(&pending), Some(2))
        .unwrap()
        .unwrap();
    assert_eq!(selected, expected_inputs);
}

#[test]
fn a_failed_job_resumes_only_its_unpublished_target_while_its_expected_pointer_is_current() {
    let pending = candidate(
        JobState::Failed,
        Some(1),
        Some(2),
        Some(GenerationState::Verified),
    );
    let expected_inputs = pending.inputs.clone();
    let selected = publish::select_resume(slice::from_ref(&pending), Some(1))
        .unwrap()
        .unwrap();
    assert_eq!(selected, expected_inputs);
}

#[test]
fn a_failed_job_with_a_published_or_stale_target_is_refused_and_named() {
    let published = candidate(
        JobState::Failed,
        Some(1),
        Some(2),
        Some(GenerationState::Published),
    );
    let job = published.id;
    let error = publish::select_resume(slice::from_ref(&published), Some(2)).unwrap_err();
    assert!(matches!(error, Failure::Refused(_)));
    assert!(error.to_string().contains(&job.to_string()));

    let stale = candidate(
        JobState::Failed,
        Some(1),
        Some(3),
        Some(GenerationState::Verified),
    );
    let job = stale.id;
    let error = publish::select_resume(&[stale], Some(2)).unwrap_err();
    assert!(matches!(error, Failure::Refused(_)));
    assert!(error.to_string().contains(&job.to_string()));
    assert!(error.to_string().contains("generation 1"));
    assert!(error.to_string().contains("generation 2 is published"));
    assert!(!error.to_string().contains("Some("));
}

#[test]
fn multiple_matching_jobs_are_refused_and_named() {
    let first = candidate(
        JobState::Failed,
        Some(2),
        Some(5),
        Some(GenerationState::Verified),
    );
    let second = candidate(JobState::Running, Some(2), None, None);
    let first_id = first.id;
    let second_id = second.id;
    let error = publish::select_resume(&[first, second], Some(2)).unwrap_err();
    assert!(matches!(error, Failure::Refused(_)));
    assert!(error.to_string().contains(&first_id.to_string()));
    assert!(error.to_string().contains(&second_id.to_string()));
}
