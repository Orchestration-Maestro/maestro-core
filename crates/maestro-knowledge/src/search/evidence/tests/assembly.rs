use super::super::assemble::{
    assemble_blocking, assemble_blocking_sequential,
    ledger::{DuplicateLedgerError, duplicate_ledger},
};
use super::super::{EvidenceCounter, EvidenceError, assemble_evidence};
use super::support::{control, evidence_input, exact_evidence_input, fixture};
use crate::prepare::tests::scratch::{
    clear_chunk_set_manifest, corrupt_artifact, replace_chunk_set_manifest,
};
use maestro_canonicalization::TokenCounter;
use maestro_kernel::{
    chunk_set::NewChunkSet,
    evidence::{Inventory, RouteStatus},
    retrieval::ReadControl,
    scope::{Right, Scope},
};
use std::{
    collections::BTreeSet,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    time::{Duration, Instant},
};
use tokio::{task::spawn_blocking, time::Instant as TokioInstant};

#[test]
fn parallel_source_loads_emit_the_same_bundle_bytes_as_sequential_loads() {
    let fixture = fixture(&[
        ("first.md", "# First\n\nA distinct first source passage.\n"),
        (
            "second.md",
            "# Second\n\nA different second source passage.\n",
        ),
    ]);
    let input = evidence_input(&fixture, "What do the source documents contain?");
    let sequential = assemble_blocking_sequential(
        &fixture.database,
        &input,
        &EvidenceCounter::Utf8Bytes,
        &control(),
    )
    .unwrap();
    let parallel = assemble_blocking(
        &fixture.database,
        &input,
        &EvidenceCounter::Utf8Bytes,
        &control(),
    )
    .unwrap();

    assert_eq!(
        serde_json::to_vec(&parallel).unwrap(),
        serde_json::to_vec(&sequential).unwrap()
    );
}

struct BlockingCounter {
    entered: Sender<()>,
    release: Mutex<Receiver<()>>,
    finished: Sender<()>,
    token_calls: Arc<AtomicUsize>,
    verify_calls: Arc<AtomicUsize>,
}

impl TokenCounter for BlockingCounter {
    fn contract_id(&self) -> &'static str {
        "test-blocking-counter/1"
    }

    fn verify(&self) -> Result<(), maestro_canonicalization::Error> {
        self.verify_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    fn token_ids(&self, _: &str) -> Result<Vec<u32>, maestro_canonicalization::Error> {
        if self.token_calls.fetch_add(1, Ordering::Relaxed) == 0 {
            self.entered.send(()).unwrap();
            self.release.lock().unwrap().recv().unwrap();
        }
        Ok(vec![1])
    }
}

impl Drop for BlockingCounter {
    fn drop(&mut self) {
        let _ = self.finished.send(());
    }
}

async fn wait_for(receiver: Receiver<()>) {
    spawn_blocking(move || receiver.recv().unwrap())
        .await
        .unwrap();
}

struct BlockingCounterRun {
    counter: EvidenceCounter,
    entered: Receiver<()>,
    release: Sender<()>,
    finished: Receiver<()>,
    token_calls: Arc<AtomicUsize>,
    verify_calls: Arc<AtomicUsize>,
}

fn blocking_counter() -> BlockingCounterRun {
    let (entered, entered_rx) = mpsc::channel();
    let (release_tx, release) = mpsc::channel();
    let (finished, finished_rx) = mpsc::channel();
    let token_calls = Arc::new(AtomicUsize::new(0));
    let verify_calls = Arc::new(AtomicUsize::new(0));
    let counter = BlockingCounter {
        entered,
        release: Mutex::new(release),
        finished,
        token_calls: token_calls.clone(),
        verify_calls: verify_calls.clone(),
    };
    BlockingCounterRun {
        counter: EvidenceCounter::Exact(Arc::new(counter)),
        entered: entered_rx,
        release: release_tx,
        finished: finished_rx,
        token_calls,
        verify_calls,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn returns_authoritative_source_bytes_and_echoes_the_complete_budget() {
    let fixture = fixture(&[(
        "guide.md",
        "# Guide\n\nThe canonical source text is authoritative.\n",
    )]);
    let input = evidence_input(&fixture, "What does the guide document say?");
    let database = Arc::new(fixture.database);

    let bundle = assemble_evidence(database, input.clone(), EvidenceCounter::Utf8Bytes)
        .await
        .unwrap();

    assert_eq!(bundle.query, input.query);
    assert_eq!(bundle.generation, input.generation.id);
    assert_eq!(bundle.request_budget, Some(input.budget));
    assert_eq!(bundle.routes, input.routes);
    assert_eq!(bundle.inventory, input.inventory);
    assert!(bundle.passages.iter().any(|passage| {
        passage
            .text
            .contains("canonical source text is authoritative")
    }));
    assert!(
        bundle
            .passages
            .iter()
            .all(|passage| !passage.text.contains("untrusted candidate text"))
    );
    assert_eq!(
        bundle.budget.counter.as_deref(),
        Some("evidence-utf8-bytes/1")
    );
    assert!(bundle.budget.estimated);
    assert_eq!(
        bundle.budget.evidence_tokens,
        u32::try_from(serde_json::to_vec(&bundle.passages).unwrap().len()).unwrap()
    );
    assert_eq!(bundle.trace.len(), bundle.passages.len());
    assert!(bundle.trace.iter().all(|trace| trace.routes == ["lexical"]));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_unversioned_single_document_has_no_latest_order_gap() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nA source passage.\n")]);
    let input = evidence_input(&fixture, "What does the guide document say?");

    let bundle = assemble_evidence(
        Arc::new(fixture.database),
        input,
        EvidenceCounter::Utf8Bytes,
    )
    .await
    .unwrap();

    assert_eq!(bundle.known_gaps, Vec::<String>::new());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn an_inventory_is_echoed_without_inventing_supporting_passages() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nA source passage.\n")]);
    let mut input = evidence_input(&fixture, "How many documents exist?");
    input.ranked.clear();
    input.inventory = Some(Inventory::DocumentsBySet {
        set_filter: None,
        total_documents: 0,
        sets: Vec::new(),
    });
    input
        .routes
        .insert("structured".to_owned(), RouteStatus::Ok);
    let database = Arc::new(fixture.database);

    let bundle = assemble_evidence(database, input.clone(), EvidenceCounter::Utf8Bytes)
        .await
        .unwrap();

    assert_eq!(bundle.inventory, input.inventory);
    assert!(bundle.passages.is_empty());
    assert!(bundle.trace.is_empty());
    let inventory_gap = concat!(
        "No source passages were returned; inventory totals are independent ",
        "of supporting passages."
    );
    assert_eq!(bundle.known_gaps, [inventory_gap]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn changed_read_grants_stop_assembly_before_source_expansion() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nPrivate source.\n")]);
    let input = evidence_input(&fixture, "What is in the guide?");
    let workspace: Scope = "workspace/default".parse().unwrap();
    fixture
        .database
        .revoke("tester", &workspace, Right::Read, "test")
        .unwrap();
    let database = Arc::new(fixture.database);

    let error = assemble_evidence(database, input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap_err();

    assert!(matches!(error, EvidenceError::PermissionsChanged));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expired_deadlines_and_inconsistent_understanding_are_rejected() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nSource.\n")]);
    let mut input = evidence_input(&fixture, "What is in the guide?");
    input.deadline = TokioInstant::now() - Duration::from_millis(1);
    let database = Arc::new(fixture.database);

    let blocked = blocking_counter();
    let error = assemble_evidence(database.clone(), input.clone(), blocked.counter)
        .await
        .unwrap_err();
    assert!(matches!(error, EvidenceError::TimedOut));
    assert_eq!(blocked.verify_calls.load(Ordering::Relaxed), 0);
    assert_eq!(blocked.token_calls.load(Ordering::Relaxed), 0);

    input.deadline = TokioInstant::now() + Duration::from_secs(1);
    input.understood.normalized = "a different query".to_owned();
    let error = assemble_evidence(database, input, EvidenceCounter::Utf8Bytes)
        .await
        .unwrap_err();
    assert!(matches!(error, EvidenceError::InvalidRequest(_)));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn caller_cancellation_stops_after_a_blocking_counter_returns() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nSource passage.\n")]);
    let input = exact_evidence_input(&fixture, "What is in the guide?");
    let database = Arc::new(fixture.database);
    let blocked = blocking_counter();
    let task = tokio::spawn(assemble_evidence(database, input, blocked.counter));

    wait_for(blocked.entered).await;
    assert_eq!(tokio::spawn(async { 7 }).await.unwrap(), 7);
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    blocked.release.send(()).unwrap();
    wait_for(blocked.finished).await;

    assert_eq!(blocked.token_calls.load(Ordering::Relaxed), 1);
    assert_eq!(blocked.verify_calls.load(Ordering::Relaxed), 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_blocked_counter_cannot_outlive_the_inherited_deadline() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nSource passage.\n")]);
    let mut input = exact_evidence_input(&fixture, "What is in the guide?");
    input.deadline = TokioInstant::now() + Duration::from_secs(2);
    let database = Arc::new(fixture.database);
    let blocked = blocking_counter();
    let task = tokio::spawn(assemble_evidence(database, input, blocked.counter));

    wait_for(blocked.entered).await;
    assert!(matches!(task.await.unwrap(), Err(EvidenceError::TimedOut)));
    blocked.release.send(()).unwrap();
    wait_for(blocked.finished).await;

    assert_eq!(blocked.token_calls.load(Ordering::Relaxed), 1);
    assert_eq!(blocked.verify_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn reads_revision_and_duplicate_allowlists_from_the_manifest() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nA small guide.\n")]);
    let control = control();
    let ledger = duplicate_ledger(
        &fixture.database,
        &fixture.scopes,
        &fixture.generation.chunk_set_id,
        &control,
    )
    .unwrap();
    let expected_revision = fixture
        .database
        .chunks(&fixture.scopes, &fixture.generation.chunk_set_id)
        .unwrap()[0]
        .revision_id
        .clone();

    assert_eq!(ledger.revisions, BTreeSet::from([expected_revision]));
    assert!(ledger.duplicates.is_empty());
    assert!(ledger.near_duplicate_groups.is_empty());
}

#[test]
fn refuses_a_ranked_chunk_from_a_manifest_duplicate_revision() {
    let fixture = fixture(&[
        ("first.md", "# First\n\nFirst source passage.\n"),
        ("second.md", "# Second\n\nSecond source passage.\n"),
    ]);
    let chunks = fixture
        .database
        .chunks(&fixture.scopes, &fixture.generation.chunk_set_id)
        .unwrap();
    let duplicate = &chunks[0];
    let representative = chunks
        .iter()
        .find(|chunk| chunk.revision_id != duplicate.revision_id)
        .expect("both revisions have prepared chunks");
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
    let mut duplicates = serde_json::Map::new();
    duplicates.insert(
        duplicate.revision_id.clone(),
        serde_json::Value::String(representative.revision_id.clone()),
    );
    manifest["duplicates"] = serde_json::Value::Object(duplicates);
    replace_chunk_set_manifest(
        &fixture.scratch,
        &fixture.database,
        &fixture.generation.chunk_set_id,
        &serde_json::to_vec(&manifest).unwrap(),
    );

    let mut input = evidence_input(&fixture, "What does the source say?");
    let mut ranked = input.ranked[0].clone();
    ranked.candidate.fused.chunk_id.clone_from(&duplicate.id);
    input.ranked = vec![ranked];
    let error = assemble_blocking(
        &fixture.database,
        &input,
        &EvidenceCounter::Utf8Bytes,
        &control(),
    )
    .unwrap_err();

    assert!(matches!(
        error,
        EvidenceError::Integrity(reason)
            if reason == "loaded candidate is outside the manifest ledger"
    ));
}

#[test]
fn refuses_building_sets_and_complete_sets_without_a_manifest() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nA small guide.\n")]);
    let building = fixture
        .database
        .begin_chunk_set(&NewChunkSet {
            id: "building",
            collection_id: &fixture.generation.collection_id,
            chunk_profile: "mapped-structural-chunks/2",
            counter_contract_id: "test-counter/1",
        })
        .unwrap();
    let control = control();
    assert!(matches!(
        duplicate_ledger(&fixture.database, &fixture.scopes, &building.id, &control),
        Err(DuplicateLedgerError::Invalid(_))
    ));

    clear_chunk_set_manifest(&fixture.scratch, &fixture.generation.chunk_set_id);
    assert!(matches!(
        duplicate_ledger(
            &fixture.database,
            &fixture.scopes,
            &fixture.generation.chunk_set_id,
            &control,
        ),
        Err(DuplicateLedgerError::Invalid(reason)) if reason.contains("manifest digest")
    ));
}

#[test]
fn manifest_artifact_and_json_errors_keep_their_error_variants() {
    let artifact_fixture = fixture(&[("guide.md", "# Guide\n\nA small guide.\n")]);
    let digest = replace_chunk_set_manifest(
        &artifact_fixture.scratch,
        &artifact_fixture.database,
        &artifact_fixture.generation.chunk_set_id,
        b"{}",
    );
    corrupt_artifact(&artifact_fixture.scratch, &digest);
    assert!(matches!(
        duplicate_ledger(
            &artifact_fixture.database,
            &artifact_fixture.scopes,
            &artifact_fixture.generation.chunk_set_id,
            &control(),
        ),
        Err(DuplicateLedgerError::Artifacts(_))
    ));

    let json_fixture = fixture(&[("guide.md", "# Guide\n\nA small guide.\n")]);
    replace_chunk_set_manifest(
        &json_fixture.scratch,
        &json_fixture.database,
        &json_fixture.generation.chunk_set_id,
        b"{",
    );
    assert!(matches!(
        duplicate_ledger(
            &json_fixture.database,
            &json_fixture.scopes,
            &json_fixture.generation.chunk_set_id,
            &control(),
        ),
        Err(DuplicateLedgerError::Json(_))
    ));
}

#[test]
fn missing_or_inaccessible_chunk_sets_are_not_visible() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nA small guide.\n")]);
    let no_scopes = fixture.database.visible("nobody").unwrap();
    let control = control();

    assert!(matches!(
        duplicate_ledger(
            &fixture.database,
            &no_scopes,
            &fixture.generation.chunk_set_id,
            &control,
        ),
        Err(DuplicateLedgerError::NotVisible)
    ));
    assert!(matches!(
        duplicate_ledger(&fixture.database, &fixture.scopes, "absent", &control),
        Err(DuplicateLedgerError::NotVisible)
    ));
}

#[test]
fn an_expired_ledger_read_is_refused_before_loading_the_chunk_set() {
    let fixture = fixture(&[("guide.md", "# Guide\n\nA small guide.\n")]);
    let control = ReadControl {
        deadline: Instant::now(),
        cancelled: Arc::new(AtomicBool::new(false)),
    };

    assert!(matches!(
        duplicate_ledger(
            &fixture.database,
            &fixture.scopes,
            &fixture.generation.chunk_set_id,
            &control,
        ),
        Err(DuplicateLedgerError::TimedOut)
    ));
}
