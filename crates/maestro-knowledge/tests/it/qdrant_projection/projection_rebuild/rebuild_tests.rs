//! Explicit replacement of a lost published projection.

use super::super::{
    backends::{Backend, fake},
    kernel::Kernel,
    models::{Embedder, embedder},
    support::{collection_of, projection, publish, state_of},
};
use maestro_kernel::{
    gateway::ModelCard,
    generation::{GenerationState, NewGeneration},
    store::Database,
};
use maestro_knowledge::index::{Error, Progress, Qdrant, RebuildGuard, Report};
use std::{collections::BTreeSet, ops::ControlFlow, sync::Arc};

#[tokio::test]
async fn again_rebuilds_a_missing_published_projection_as_a_new_generation() {
    let kernel = Kernel::with_guides(3);
    let backend = fake();
    let (qdrant, card, port) = (backend.client(), embedder(8), Embedder::default());
    let old = publish(&kernel, &backend, &port, &card).await.unwrap();
    backend.delete_collection(&old.qdrant_collection).await;
    assert_eq!(backend.alias(&old.alias).await, None);

    let mut progress = Vec::new();
    let rebuilt = projection(&kernel, &qdrant, &port, &card)
        .republish_observed(
            &kernel.chunk_set,
            RebuildGuard {
                expected_published: Some(old.generation),
                generation_watermark: old.generation,
            },
            None,
            &mut |step: &Progress| {
                progress.push(step.clone());
                ControlFlow::Continue(())
            },
        )
        .await
        .unwrap();

    assert_report_identity(&old, &rebuilt);
    assert_backend_rebuild(&backend, &kernel, &old, &rebuilt).await;
    assert_initial_progress(&progress);
}

/// Compares the replacement's generation identity with its predecessor.
fn assert_report_identity(old: &Report, rebuilt: &Report) {
    assert_ne!(rebuilt.generation, old.generation);
    assert_eq!(rebuilt.chunk_set, old.chunk_set);
    assert_eq!(rebuilt.embedding_profile, old.embedding_profile);
    assert_eq!(rebuilt.sparse_profile, old.sparse_profile);
    assert_eq!(rebuilt.points, old.points);
}

/// Checks the alias, point count and retained generation states.
async fn assert_backend_rebuild(
    backend: &Backend,
    kernel: &Kernel,
    old: &Report,
    rebuilt: &Report,
) {
    assert_eq!(
        backend.alias(&rebuilt.alias).await,
        Some(rebuilt.qdrant_collection.clone())
    );
    assert_eq!(backend.count(&rebuilt.qdrant_collection).await, old.points);
    assert_eq!(state_of(kernel, old.generation), GenerationState::Retired);
    assert_eq!(
        state_of(kernel, rebuilt.generation),
        GenerationState::Published
    );
    assert_eq!(collection_of(kernel, old.generation), old.qdrant_collection);
}

/// The first observer call records the zero-indexed replacement target.
fn assert_initial_progress(progress: &[Progress]) {
    assert!(progress.first().is_some_and(|step| step.indexed == 0));
}

#[tokio::test]
async fn again_resumes_an_empty_target_from_zero_and_reconciles_a_verified_alias_target() {
    let mut fixture = resume_fixture(fake()).await;
    let partial = interrupt_rebuild(&fixture).await;
    let resumed = resume_interrupted(&mut fixture, &partial).await;
    let current = resumed.generation;
    let saved = stop_after_alias(&mut fixture, current).await;
    let finished = reconcile_verified(&mut fixture, &saved, current).await;
    assert_eq!(finished.generation, saved.generation);
}

/// The disposable projection, kernel and target used by resume assertions.
pub(super) struct ResumeFixture {
    /// The fake server and alias/collection inspector.
    pub(super) backend: Backend,
    /// The synthetic kernel and chunk set.
    pub(super) kernel: Kernel,
    /// The library client for the fake server.
    pub(super) qdrant: Qdrant,
    /// The dense embedder card.
    pub(super) card: ModelCard,
    /// The deterministic model port.
    pub(super) port: Embedder,
    /// The originally published generation report.
    pub(super) old: Report,
    /// Point IDs from the original generation.
    pub(super) expected_ids: BTreeSet<String>,
    /// Number of chunks in the complete set.
    pub(super) chunks: usize,
}

/// Publishes the original generation and records its point IDs and chunk count.
pub(super) async fn resume_fixture(backend: Backend) -> ResumeFixture {
    let kernel = Kernel::with_guides(30);
    let qdrant = backend.client();
    let card = embedder(8);
    let port = Embedder::default();
    let old = publish(&kernel, &backend, &port, &card).await.unwrap();
    let expected_ids = backend.ids(&old.qdrant_collection).await;
    let chunks = kernel.chunks().len();
    ResumeFixture {
        backend,
        kernel,
        qdrant,
        card,
        port,
        old,
        expected_ids,
        chunks,
    }
}

/// Leaves a replacement partly indexed, with the original alias intact.
async fn interrupt_rebuild(fixture: &ResumeFixture) -> Progress {
    let mut interrupted = Vec::new();
    let stopped = projection(
        &fixture.kernel,
        &fixture.qdrant,
        &fixture.port,
        &fixture.card,
    )
    .republish_observed(
        &fixture.kernel.chunk_set,
        rebuild_guard(fixture.old.generation),
        None,
        &mut |step: &Progress| {
            interrupted.push(step.clone());
            if step.indexed > 0 {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(stopped, Error::Stopped));
    let partial = interrupted.last().unwrap().clone();
    assert_eq!(partial.indexed, 64);
    assert_eq!(
        state_of(&fixture.kernel, partial.generation),
        GenerationState::Building
    );
    assert_eq!(
        fixture.backend.alias(&fixture.old.alias).await,
        Some(fixture.old.qdrant_collection.clone())
    );
    assert_eq!(
        fixture
            .backend
            .count(&collection_of(&fixture.kernel, partial.generation))
            .await,
        64
    );
    partial
}

/// Deletes the old and partial collections, then resumes after the journaled batch.
async fn resume_interrupted(fixture: &mut ResumeFixture, partial: &Progress) -> Report {
    let calls_before_replay = fixture.port.calls().len();
    fixture
        .backend
        .delete_collection(&fixture.old.qdrant_collection)
        .await;
    fixture
        .backend
        .delete_collection(&collection_of(&fixture.kernel, partial.generation))
        .await;
    assert_eq!(fixture.backend.alias(&fixture.old.alias).await, None);
    reopen(&mut fixture.kernel);

    let mut replayed = Vec::new();
    let resumed = projection(
        &fixture.kernel,
        &fixture.qdrant,
        &fixture.port,
        &fixture.card,
    )
    .republish_observed(
        &fixture.kernel.chunk_set,
        rebuild_guard(fixture.old.generation),
        Some(partial),
        &mut |step: &Progress| {
            replayed.push(step.clone());
            ControlFlow::Continue(())
        },
    )
    .await
    .unwrap();
    assert_resumed_replacement(fixture, partial, &resumed, calls_before_replay, &replayed).await;
    resumed
}

/// Checks the replacement, replayed batch range and embedding calls.
async fn assert_resumed_replacement(
    fixture: &ResumeFixture,
    partial: &Progress,
    resumed: &Report,
    calls_before_replay: usize,
    replayed: &[Progress],
) {
    assert_eq!(resumed.generation, partial.generation);
    assert_eq!(resumed.points, u64::try_from(fixture.chunks).unwrap());
    assert_eq!(
        state_of(&fixture.kernel, fixture.old.generation),
        GenerationState::Retired
    );
    assert_eq!(
        state_of(&fixture.kernel, resumed.generation),
        GenerationState::Published
    );
    assert_eq!(
        fixture.backend.alias(&resumed.alias).await,
        Some(resumed.qdrant_collection.clone())
    );
    assert_eq!(
        fixture.backend.ids(&resumed.qdrant_collection).await,
        fixture.expected_ids
    );
    assert!(replayed.first().is_some_and(|step| step.indexed == 64));
    let replay_calls = &fixture.port.calls()[calls_before_replay..];
    assert!(replay_calls.len() >= 2);
    assert_eq!(
        replay_calls.iter().map(Vec::len).sum::<usize>(),
        fixture.chunks
    );
}

/// Stops after the replacement alias moves but before the kernel pointer changes.
pub(super) async fn stop_after_alias(fixture: &mut ResumeFixture, current: i64) -> Progress {
    let mut verified = Vec::new();
    let stopped = projection(
        &fixture.kernel,
        &fixture.qdrant,
        &fixture.port,
        &fixture.card,
    )
    .republish_observed(
        &fixture.kernel.chunk_set,
        rebuild_guard(current),
        None,
        &mut |step: &Progress| {
            verified.push(step.clone());
            if step.indexed == step.chunks {
                fixture
                    .kernel
                    .database
                    .complete_generation_search(&fixture.kernel.scopes, step.generation)
                    .unwrap();
                fixture
                    .kernel
                    .database
                    .verify_generation(step.generation, step.chunks)
                    .unwrap();
                fixture.backend.fake.as_ref().unwrap().alias_to(
                    &fixture.old.alias,
                    &collection_of(&fixture.kernel, step.generation),
                );
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(stopped, Error::Stopped));
    let saved = verified.last().unwrap().clone();
    assert_eq!(
        state_of(&fixture.kernel, saved.generation),
        GenerationState::Verified
    );
    assert_eq!(
        fixture.backend.alias(&fixture.old.alias).await,
        Some(collection_of(&fixture.kernel, saved.generation))
    );
    assert_eq!(
        fixture
            .kernel
            .database
            .published_generation(&fixture.kernel.scopes, &fixture.kernel.collection)
            .unwrap()
            .unwrap()
            .id,
        current,
        "the kernel pointer is still old at the crash boundary"
    );
    reopen(&mut fixture.kernel);
    saved
}

/// Reconciles the verified target from its saved step and checks the final ranking inputs.
async fn reconcile_verified(fixture: &mut ResumeFixture, saved: &Progress, current: i64) -> Report {
    let mut reconciled = Vec::new();
    let finished = projection(
        &fixture.kernel,
        &fixture.qdrant,
        &fixture.port,
        &fixture.card,
    )
    .republish_observed(
        &fixture.kernel.chunk_set,
        rebuild_guard(current),
        Some(saved),
        &mut |step: &Progress| {
            reconciled.push(step.clone());
            ControlFlow::Continue(())
        },
    )
    .await
    .unwrap();
    assert_eq!(
        state_of(&fixture.kernel, fixture.old.generation),
        GenerationState::Retired
    );
    assert_eq!(
        state_of(&fixture.kernel, finished.generation),
        GenerationState::Published
    );
    assert_eq!(
        fixture.backend.ids(&finished.qdrant_collection).await,
        fixture.expected_ids
    );
    assert_eq!(
        fixture.backend.count(&finished.qdrant_collection).await,
        u64::try_from(fixture.chunks).unwrap()
    );
    assert_eq!(
        fixture
            .kernel
            .database
            .generations(&fixture.kernel.scopes, &fixture.kernel.collection)
            .unwrap()
            .len(),
        3
    );
    assert!(
        reconciled.is_empty(),
        "verified target is checked without another build"
    );
    fixture
        .backend
        .cleanup(
            &fixture.kernel.collection,
            [fixture.old.generation, current, finished.generation],
        )
        .await;
    finished
}

/// Reopens the database after simulating a process restart.
pub(super) fn reopen(kernel: &mut Kernel) {
    let directory = kernel.database_path().parent().unwrap().to_path_buf();
    kernel.database = Arc::new(Database::open_in(&directory).unwrap());
}

/// Uses the same pointer and watermark for a test-created replacement.
pub(super) fn rebuild_guard(generation: i64) -> RebuildGuard {
    RebuildGuard {
        expected_published: Some(generation),
        generation_watermark: generation,
    }
}

#[tokio::test]
async fn again_refuses_when_the_published_pointer_changed_before_rebuild() {
    let backend = fake();
    let kernel = Kernel::with_guides(3);
    let (qdrant, card, port) = (backend.client(), embedder(8), Embedder::default());
    let old = publish(&kernel, &backend, &port, &card).await.unwrap();
    let second = kernel
        .database
        .create_generation(&NewGeneration {
            collection_id: kernel.collection.clone(),
            chunk_set_id: kernel.chunk_set.clone(),
            embedding_profile: old.embedding_profile.clone(),
            sparse_profile: old.sparse_profile.clone(),
        })
        .unwrap();
    kernel.database.verify_generation(second.id, 0).unwrap();
    kernel.database.publish_generation(second.id).unwrap();

    let error = projection(&kernel, &qdrant, &port, &card)
        .republish_observed(
            &kernel.chunk_set,
            rebuild_guard(old.generation),
            None,
            &mut |_| ControlFlow::Continue(()),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, Error::PublishedChanged { .. }), "{error}");
    assert!(!backend.exists(&collection_of(&kernel, second.id)).await);
    backend
        .cleanup(&kernel.collection, [old.generation, second.id])
        .await;
}
