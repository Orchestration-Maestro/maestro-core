//! Crash boundaries around alias and kernel publication.

use super::super::{
    backends::{backends, fake},
    support::{collection_of, projection, state_of},
};
use super::rebuild_tests::{
    ResumeFixture, rebuild_guard, reopen, resume_fixture, stop_after_alias,
};
use maestro_kernel::generation::{GenerationState, NewGeneration};
use maestro_knowledge::index::{Error, Progress};
use std::ops::ControlFlow;

#[tokio::test]
async fn failed_alias_switch_reuses_its_verified_target_on_retry() {
    let backend = fake();
    let mut fixture = resume_fixture(backend).await;
    let current = fixture.old.generation;
    fixture.backend.refuse_create_alias();
    let mut progress = Vec::new();
    let error = projection(
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
            progress.push(step.clone());
            ControlFlow::Continue(())
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(error, Error::Qdrant(_)));
    let saved = progress.last().unwrap().clone();
    assert_eq!(
        state_of(&fixture.kernel, saved.generation),
        GenerationState::Verified
    );
    assert_eq!(
        fixture.backend.alias(&fixture.old.alias).await,
        Some(fixture.old.qdrant_collection.clone())
    );
    reopen(&mut fixture.kernel);
    let retry = projection(
        &fixture.kernel,
        &fixture.qdrant,
        &fixture.port,
        &fixture.card,
    )
    .republish_observed(
        &fixture.kernel.chunk_set,
        rebuild_guard(current),
        Some(&saved),
        &mut |_| ControlFlow::Continue(()),
    )
    .await
    .unwrap();
    assert_eq!(retry.generation, saved.generation);
    assert_eq!(
        state_of(&fixture.kernel, retry.generation),
        GenerationState::Published
    );
    assert_eq!(
        fixture.backend.alias(&retry.alias).await,
        Some(retry.qdrant_collection.clone())
    );
    assert_eq!(
        fixture
            .kernel
            .database
            .generations(&fixture.kernel.scopes, &fixture.kernel.collection)
            .unwrap()
            .len(),
        2
    );
    fixture
        .backend
        .cleanup(
            &fixture.kernel.collection,
            [fixture.old.generation, retry.generation],
        )
        .await;
}

#[tokio::test]
async fn failed_kernel_publish_reuses_the_verified_alias_target_on_retry() {
    let backend = fake();
    let mut fixture = resume_fixture(backend).await;
    let current = fixture.old.generation;
    let connection = rusqlite::Connection::open(fixture.kernel.database_path()).unwrap();
    connection
        .execute_batch(
            "CREATE TRIGGER reject_publish BEFORE UPDATE OF state ON generations
             WHEN NEW.state = 'published'
             BEGIN SELECT RAISE(ABORT, 'injected publication failure'); END;",
        )
        .unwrap();
    let mut progress = Vec::new();
    let error = projection(
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
            progress.push(step.clone());
            ControlFlow::Continue(())
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(error, Error::Generation(_)));
    assert_eq!(
        state_of(&fixture.kernel, current),
        GenerationState::Published
    );
    let saved = progress.last().unwrap().clone();
    assert_eq!(
        state_of(&fixture.kernel, saved.generation),
        GenerationState::Verified
    );
    assert_eq!(
        fixture.backend.alias(&fixture.old.alias).await,
        Some(collection_of(&fixture.kernel, saved.generation))
    );
    connection
        .execute_batch("DROP TRIGGER reject_publish;")
        .unwrap();
    drop(connection);
    reopen(&mut fixture.kernel);
    let retry = projection(
        &fixture.kernel,
        &fixture.qdrant,
        &fixture.port,
        &fixture.card,
    )
    .republish_observed(
        &fixture.kernel.chunk_set,
        rebuild_guard(current),
        Some(&saved),
        &mut |_| ControlFlow::Continue(()),
    )
    .await
    .unwrap();
    assert_eq!(retry.generation, saved.generation);
    assert_eq!(
        state_of(&fixture.kernel, retry.generation),
        GenerationState::Published
    );
    assert_eq!(
        fixture.backend.alias(&retry.alias).await,
        Some(retry.qdrant_collection.clone())
    );
    assert_eq!(
        fixture
            .kernel
            .database
            .generations(&fixture.kernel.scopes, &fixture.kernel.collection)
            .unwrap()
            .len(),
        2
    );
    fixture
        .backend
        .cleanup(
            &fixture.kernel.collection,
            [fixture.old.generation, retry.generation],
        )
        .await;
}

struct CompetingGeneration<'a> {
    fixture: &'a ResumeFixture,
    generation: Option<i64>,
}

impl CompetingGeneration<'_> {
    fn observe(&mut self, step: &Progress) {
        if self.generation.is_none() && step.indexed == step.chunks {
            self.generation = Some(publish_competing_generation(self.fixture));
        }
    }
}

fn publish_competing_generation(fixture: &ResumeFixture) -> i64 {
    let generation = fixture
        .kernel
        .database
        .create_generation(&NewGeneration {
            collection_id: fixture.old.collection.clone(),
            chunk_set_id: fixture.kernel.chunk_set.clone(),
            embedding_profile: fixture.old.embedding_profile.clone(),
            sparse_profile: fixture.old.sparse_profile.clone(),
        })
        .unwrap();
    fixture
        .kernel
        .database
        .verify_generation(generation.id, 0)
        .unwrap();
    fixture
        .kernel
        .database
        .publish_generation(generation.id)
        .unwrap();
    generation.id
}

#[tokio::test]
async fn pointer_change_after_verification_refuses_commit_before_moving_the_alias() {
    for backend in
        backends("pointer_change_after_verification_refuses_commit_before_moving_the_alias")
    {
        let fixture = resume_fixture(backend).await;
        let current = fixture.old.generation;
        let mut competing = CompetingGeneration {
            fixture: &fixture,
            generation: None,
        };
        let result = projection(
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
                competing.observe(step);
                ControlFlow::Continue(())
            },
        )
        .await
        .unwrap_err();
        let competing = competing.generation.unwrap();
        assert!(matches!(
            result,
            Error::PublishedChanged {
                expected: Some(expected),
                found: Some(found),
            } if expected == current && found == competing
        ));
        assert_eq!(
            state_of(&fixture.kernel, competing),
            GenerationState::Published
        );
        assert_eq!(
            fixture.backend.alias(&fixture.old.alias).await,
            Some(fixture.old.qdrant_collection.clone())
        );
        let generations = fixture
            .kernel
            .database
            .generations(&fixture.kernel.scopes, &fixture.kernel.collection)
            .unwrap();
        assert_eq!(generations.len(), 3);
        fixture
            .backend
            .cleanup(
                &fixture.kernel.collection,
                generations.into_iter().map(|generation| generation.id),
            )
            .await;
    }
}

#[tokio::test]
async fn a_published_progress_target_is_verified_and_reconciled_without_another_generation() {
    for backend in backends(
        "a_published_progress_target_is_verified_and_reconciled_without_another_generation",
    ) {
        let mut fixture = resume_fixture(backend).await;
        let current = fixture.old.generation;
        let saved = stop_after_alias(&mut fixture, current).await;
        fixture
            .kernel
            .database
            .publish_generation(saved.generation)
            .unwrap();
        fixture
            .backend
            .point_alias(&fixture.old.alias, &fixture.old.qdrant_collection)
            .await;
        reopen(&mut fixture.kernel);
        let calls_before_reconcile = fixture.port.calls().len();
        let mut observed = Vec::new();
        let reconciled = projection(
            &fixture.kernel,
            &fixture.qdrant,
            &fixture.port,
            &fixture.card,
        )
        .republish_observed(
            &fixture.kernel.chunk_set,
            rebuild_guard(current),
            Some(&saved),
            &mut |step: &Progress| {
                observed.push(step.clone());
                ControlFlow::Continue(())
            },
        )
        .await
        .unwrap();

        assert_eq!(reconciled.generation, saved.generation);
        assert_eq!(
            state_of(&fixture.kernel, reconciled.generation),
            GenerationState::Published
        );
        assert_eq!(
            state_of(&fixture.kernel, fixture.old.generation),
            GenerationState::Retired
        );
        assert_eq!(
            fixture.backend.alias(&reconciled.alias).await,
            Some(reconciled.qdrant_collection.clone())
        );
        assert_eq!(
            fixture.backend.ids(&reconciled.qdrant_collection).await,
            fixture.expected_ids
        );
        assert!(observed.is_empty());
        assert_eq!(fixture.port.calls().len(), calls_before_reconcile);
        let generations = fixture
            .kernel
            .database
            .generations(&fixture.kernel.scopes, &fixture.kernel.collection)
            .unwrap();
        assert_eq!(generations.len(), 2);
        fixture
            .backend
            .cleanup(
                &fixture.kernel.collection,
                generations.into_iter().map(|generation| generation.id),
            )
            .await;
    }
}
