use super::super::assemble::assemble_blocking;
use super::{
    super::{EvidenceCounter, EvidenceError, assemble_evidence},
    support::{Fixture, control, evidence_input, fixture},
};
use crate::prepare::tests::scratch::{
    corrupt_artifact, quarantine_revision, replace_chunk_set_manifest, replace_revision_canonical,
    revision_of,
};
use maestro_canonicalization::{CanonicalDocument, TokenCounter, ValidationStatus};
use maestro_kernel::{
    generation::{GenerationState, NewGeneration},
    scope::{Right, Scope},
};
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
    },
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
        (
            "revision",
            "canonical artifact identity or status is invalid",
        ),
        (
            "content hash",
            "canonical artifact identity or status is invalid",
        ),
        (
            "source reference hash",
            "canonical artifact identity or status is invalid",
        ),
        (
            "validation status",
            "canonical artifact identity or status is invalid",
        ),
        ("structure", "canonical source spans or links are invalid"),
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
        } else if field == "revision" {
            canonical.revision_id = "another-revision".to_owned();
        } else if field == "content hash" {
            canonical.content_hash = "sha256:wrong".to_owned();
            canonical.original_markdown_reference.content_hash = canonical.content_hash.clone();
        } else if field == "source reference hash" {
            canonical.original_markdown_reference.content_hash = "sha256:wrong".to_owned();
        } else if field == "validation status" {
            canonical.validation_status = ValidationStatus::Failed;
        } else {
            canonical.sections.first_mut().unwrap().parent_section_id =
                Some("missing-section".to_owned());
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

fn table_chunk_id(fixture: &Fixture, revision: &str, markdown: &str) -> String {
    fixture
        .database
        .chunks(&fixture.scopes, &fixture.generation.chunk_set_id)
        .unwrap()
        .into_iter()
        .find(|chunk| {
            chunk.revision_id == revision
                && markdown
                    .get(chunk.span.start..chunk.span.end)
                    .is_some_and(|text| text.contains("| Agent | Port |"))
        })
        .expect("near duplicate revision has a table chunk")
        .id
        .clone()
}

#[test]
fn near_duplicate_rows_outside_the_manifest_allowlist_do_not_join_families() {
    let shared = (0..200)
        .map(|index| format!("sharedword{index}"))
        .collect::<Vec<_>>()
        .join(" ");
    let old = format!(
        concat!(
            "# Guide\n\n{shared}\n\n",
            "| Entity | Attribute | Value |\n| --- | --- | --- |\n",
            "| Agent | Port | 7005 |\n"
        ),
        shared = shared
    );
    let current = old
        .replace("sharedword100 ", "changedword100 ")
        .replace("7005", "7006");
    let fixture = fixture(&[("old.md", &old), ("current.md", &current)]);
    let old_revision = revision_of(&fixture.database, &fixture.scopes, "old.md");
    let current_revision = revision_of(&fixture.database, &fixture.scopes, "current.md");
    assert!(
        fixture
            .database
            .near_duplicates(&fixture.scopes, &old_revision)
            .unwrap()
            .len()
            > 1
    );
    let old_groups = fixture
        .database
        .near_duplicates(&fixture.scopes, &old_revision)
        .unwrap()
        .into_iter()
        .map(|member| member.group_id)
        .collect::<BTreeSet<_>>();
    let current_groups = fixture
        .database
        .near_duplicates(&fixture.scopes, &current_revision)
        .unwrap()
        .into_iter()
        .map(|member| member.group_id)
        .collect::<BTreeSet<_>>();
    assert!(
        old_groups
            .iter()
            .any(|group| current_groups.contains(group)),
        "the prepared revisions must share a near-duplicate group"
    );
    let set = fixture
        .database
        .chunk_set(&fixture.scopes, &fixture.generation.chunk_set_id)
        .unwrap()
        .unwrap();
    let bytes = fixture
        .database
        .get(set.manifest_digest.as_ref().unwrap())
        .unwrap();
    let mut manifest: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(
        !manifest["near_duplicate_groups"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let mut input = evidence_input(&fixture, "What port does the Agent use?");
    let mut old_ranked = input.ranked[0].clone();
    old_ranked.candidate.fused.chunk_id = table_chunk_id(&fixture, &old_revision, &old);
    let mut current_ranked = old_ranked.clone();
    current_ranked.candidate.fused.chunk_id = table_chunk_id(&fixture, &current_revision, &current);
    input.ranked = vec![old_ranked, current_ranked];

    let allowed_bundle = assemble_blocking(
        &fixture.database,
        &input,
        &EvidenceCounter::Utf8Bytes,
        &control(),
    )
    .unwrap();
    assert!(
        !allowed_bundle.conflicts.is_empty(),
        "authorized near-duplicate groups must expose the table conflict"
    );

    manifest["near_duplicate_groups"] = serde_json::Value::Array(Vec::new());
    replace_chunk_set_manifest(
        &fixture.scratch,
        &fixture.database,
        &fixture.generation.chunk_set_id,
        &serde_json::to_vec(&manifest).unwrap(),
    );
    let bundle = assemble_blocking(
        &fixture.database,
        &input,
        &EvidenceCounter::Utf8Bytes,
        &control(),
    )
    .unwrap();
    assert!(bundle.conflicts.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejects_each_changed_pinned_generation_profile() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nAn authoritative source.\n")]);
    let base = evidence_input(&fixture, "What does the guide say?");
    let database = Arc::new(fixture.database);

    let mut collection = base.clone();
    collection.generation.collection_id = "other-collection".to_owned();
    let mut embedding = base.clone();
    embedding.generation.embedding_profile = "other-embedding".to_owned();
    let mut sparse = base.clone();
    sparse.generation.sparse_profile = "other-sparse".to_owned();
    for (field, input) in [
        ("collection", collection),
        ("embedding", embedding),
        ("sparse", sparse),
    ] {
        let error = assemble_evidence(database.clone(), input, EvidenceCounter::Utf8Bytes)
            .await
            .unwrap_err();
        assert!(
            matches!(
                error,
                EvidenceError::Integrity(reason) if reason == "pinned generation identity changed"
            ),
            "{field}"
        );
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
