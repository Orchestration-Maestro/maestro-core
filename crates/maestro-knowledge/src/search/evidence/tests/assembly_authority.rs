use super::{
    super::{EvidenceCounter, EvidenceError, assemble_evidence},
    support::{evidence_input, fixture},
};
use crate::prepare::tests::scratch::{
    corrupt_artifact, quarantine_revision, replace_revision_canonical, revision_of,
};
use maestro_canonicalization::{CanonicalDocument, TokenCounter};
use maestro_kernel::{
    generation::{GenerationState, NewGeneration},
    scope::{Right, Scope},
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
    mpsc::{self, Receiver, Sender},
};
use tokio::task::spawn_blocking;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn source_corruption_and_canonical_refusals_keep_exact_error_variants() {
    let initial_fixture = fixture(&[("guide.md", "# Guide\n\nAn authoritative source.\n")]);
    let revision_id = revision_of(
        &initial_fixture.database,
        &initial_fixture.scopes,
        "guide.md",
    );
    let revision = initial_fixture
        .database
        .revision(&initial_fixture.scopes, &revision_id)
        .unwrap()
        .unwrap();
    let input = evidence_input(&initial_fixture, "What does the guide say?");
    corrupt_artifact(&initial_fixture.scratch, &revision.original_digest);
    let error = assemble_evidence(
        Arc::new(initial_fixture.database),
        input,
        EvidenceCounter::Utf8Bytes,
    )
    .await
    .unwrap_err();
    assert!(matches!(error, EvidenceError::Store(_)));

    for (field, expected) in [
        (
            "identity",
            "canonical artifact identity or status is invalid",
        ),
        ("replay", "canonical document replay found an error"),
    ] {
        let fixture = fixture(&[("guide.md", "# Guide\n\nAn authoritative source.\n")]);
        let revision_id = revision_of(&fixture.database, &fixture.scopes, "guide.md");
        let revision = fixture
            .database
            .revision(&fixture.scopes, &revision_id)
            .unwrap()
            .unwrap();
        let mut canonical: CanonicalDocument =
            serde_json::from_slice(&fixture.database.get(&revision.canonical_digest).unwrap())
                .unwrap();
        if field == "identity" {
            canonical.document_id = "another-document".to_owned();
        } else {
            canonical.sections.first_mut().unwrap().title = "changed heading".to_owned();
        }
        replace_revision_canonical(
            &fixture.scratch,
            &fixture.database,
            &revision_id,
            &serde_json::to_vec(&canonical).unwrap(),
        );
        let input = evidence_input(&fixture, "What does the guide say?");
        let error = assemble_evidence(
            Arc::new(fixture.database),
            input,
            EvidenceCounter::Utf8Bytes,
        )
        .await
        .unwrap_err();
        match error {
            EvidenceError::Integrity(reason) => assert_eq!(reason, expected),
            other => panic!("{field}: expected Integrity, got {other:?}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn hidden_generations_and_changed_pinned_chunk_sets_are_refused() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nAn authoritative source.\n")]);
    let mut hidden_input = evidence_input(&fixture, "What does the guide say?");
    hidden_input.principal = "nobody".to_owned();
    hidden_input.scopes = Arc::new(fixture.database.visible("nobody").unwrap());
    let mut changed_input = evidence_input(&fixture, "What does the guide say?");
    changed_input.generation.chunk_set_id = "different-set".to_owned();
    let database = Arc::new(fixture.database);

    let hidden = assemble_evidence(database.clone(), hidden_input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap_err();
    assert!(matches!(hidden, EvidenceError::NotVisible));

    let changed = assemble_evidence(database, changed_input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap_err();
    match changed {
        EvidenceError::Integrity(reason) => {
            assert_eq!(reason, "pinned generation identity changed");
        }
        other => panic!("expected pinned-identity Integrity, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retired_generations_remain_readable_and_building_generations_do_not() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nAn authoritative source.\n")]);
    let old_generation = fixture.generation.clone();
    let scopes = fixture.scopes.clone();
    let mut retired_input = evidence_input(&fixture, "What does the guide say?");
    let mut building_input = evidence_input(&fixture, "What does the guide say?");
    let database = Arc::new(fixture.database);
    let new_generation = database
        .create_generation(&NewGeneration {
            collection_id: old_generation.collection_id.clone(),
            chunk_set_id: old_generation.chunk_set_id.clone(),
            embedding_profile: old_generation.embedding_profile.clone(),
            sparse_profile: old_generation.sparse_profile.clone(),
        })
        .unwrap();
    let building_generation = database
        .create_generation(&NewGeneration {
            collection_id: old_generation.collection_id.clone(),
            chunk_set_id: old_generation.chunk_set_id.clone(),
            embedding_profile: old_generation.embedding_profile.clone(),
            sparse_profile: old_generation.sparse_profile.clone(),
        })
        .unwrap();
    database
        .verify_generation(new_generation.id, old_generation.point_count.unwrap_or(0))
        .unwrap();
    database.publish_generation(new_generation.id).unwrap();
    assert_eq!(
        database
            .generation(&scopes, old_generation.id)
            .unwrap()
            .unwrap()
            .state,
        GenerationState::Retired
    );
    retired_input.generation = old_generation;
    building_input.generation = building_generation;

    let retired = assemble_evidence(database.clone(), retired_input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap();
    assert_eq!(retired.passages.len(), 1);

    let building = assemble_evidence(database, building_input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap_err();
    assert!(matches!(building, EvidenceError::NotVisible));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grants_added_before_assembly_are_detected() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nAn authoritative source.\n")]);
    let input = evidence_input(&fixture, "What does the guide say?");
    let database = Arc::new(fixture.database);
    let added_scope: Scope = "workspace/late-grant".parse().unwrap();
    database
        .grant("tester", &added_scope, Right::Read, "test")
        .unwrap();

    let error = assemble_evidence(database, input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap_err();

    assert!(matches!(error, EvidenceError::PermissionsChanged));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn late_grant_or_eligibility_changes_abort_before_delivery() {
    let first_fixture = fixture(&[("guide.md", "# Guide\n\nAn authoritative source.\n")]);
    let input = evidence_input(&first_fixture, "What does the guide say?");
    let database = Arc::new(first_fixture.database);
    let run = blocked_verify_counter();
    let task = tokio::spawn(assemble_evidence(database.clone(), input, run.counter));
    wait_for(run.entered).await;
    let added_scope: Scope = "workspace/late-delivery".parse().unwrap();
    database
        .grant("tester", &added_scope, Right::Read, "test")
        .unwrap();
    run.release.send(()).unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(EvidenceError::PermissionsChanged)
    ));

    let fixture = fixture(&[("guide.md", "# Guide\n\nAn authoritative source.\n")]);
    let input = evidence_input(&fixture, "What does the guide say?");
    let revision_id = revision_of(&fixture.database, &fixture.scopes, "guide.md");
    let run = blocked_verify_counter();
    let task = tokio::spawn(assemble_evidence(
        Arc::new(fixture.database),
        input,
        run.counter,
    ));
    wait_for(run.entered).await;
    quarantine_revision(&fixture.scratch, &revision_id);
    run.release.send(()).unwrap();
    match task.await.unwrap() {
        Err(EvidenceError::Integrity(reason)) => {
            assert_eq!(reason, "evidence eligibility changed during assembly");
        }
        other => panic!("expected eligibility Integrity, got {other:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_panicking_counter_is_reported_as_worker_failed() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nAn authoritative source.\n")]);
    let input = evidence_input(&fixture, "What does the guide say?");

    let error = assemble_evidence(
        Arc::new(fixture.database),
        input,
        EvidenceCounter::Exact(Arc::new(PanickingCounter)),
    )
    .await
    .unwrap_err();

    assert!(matches!(error, EvidenceError::WorkerFailed));
}

struct PanickingCounter;

impl TokenCounter for PanickingCounter {
    fn contract_id(&self) -> &'static str {
        "panicking-counter/1"
    }

    fn verify(&self) -> Result<(), maestro_canonicalization::Error> {
        panic!("test counter panic");
    }

    fn token_ids(&self, _: &str) -> Result<Vec<u32>, maestro_canonicalization::Error> {
        Ok(vec![1])
    }
}

struct BlockedVerifyCounter {
    calls: AtomicUsize,
    entered: Sender<()>,
    release: Mutex<Receiver<()>>,
}

impl TokenCounter for BlockedVerifyCounter {
    fn contract_id(&self) -> &'static str {
        "blocked-verify-counter/1"
    }

    fn verify(&self) -> Result<(), maestro_canonicalization::Error> {
        if self.calls.fetch_add(1, Ordering::Relaxed) == 1 {
            self.entered.send(()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
        }
        Ok(())
    }

    fn token_ids(&self, _: &str) -> Result<Vec<u32>, maestro_canonicalization::Error> {
        Ok(vec![1])
    }
}

struct BlockedVerifyRun {
    counter: EvidenceCounter,
    entered: Receiver<()>,
    release: Sender<()>,
}

fn blocked_verify_counter() -> BlockedVerifyRun {
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    BlockedVerifyRun {
        counter: EvidenceCounter::Exact(Arc::new(BlockedVerifyCounter {
            calls: AtomicUsize::new(0),
            entered,
            release: Mutex::new(release_rx),
        })),
        entered: entered_rx,
        release,
    }
}

async fn wait_for(receiver: Receiver<()>) {
    spawn_blocking(move || receiver.recv().unwrap())
        .await
        .unwrap();
}
