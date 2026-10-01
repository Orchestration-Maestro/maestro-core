//! The published alias a guarded replacement restores or refuses to leave.

use super::super::{
    backends::fake,
    support::{collection_of, projection, state_of},
};
use super::rebuild_tests::{rebuild_guard, resume_fixture, stop_after_alias};
use maestro_kernel::generation::GenerationState;
use maestro_knowledge::index::Error;
use std::ops::ControlFlow;

#[tokio::test]
async fn a_failed_recheck_of_a_verified_replacement_restores_the_published_alias() {
    let mut fixture = resume_fixture(fake()).await;
    let current = fixture.old.generation;
    let saved = stop_after_alias(&mut fixture, current).await;
    let target = collection_of(&fixture.kernel, saved.generation);
    fixture.backend.intrude(&target, 8).await;

    let error = projection(
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
    .unwrap_err();

    assert!(
        matches!(error, Error::Unverified { generation, .. } if generation == saved.generation),
        "{error}"
    );
    assert_eq!(
        fixture.backend.alias(&fixture.old.alias).await,
        Some(fixture.old.qdrant_collection.clone())
    );
}

#[tokio::test]
async fn an_alias_moved_elsewhere_while_committing_refuses_the_replacement() {
    let fixture = resume_fixture(fake()).await;
    let stray = format!("maestro-{}-stray", fixture.kernel.collection);
    fixture
        .backend
        .fake
        .as_ref()
        .unwrap()
        .redirect_next_alias_to(&stray);

    let error = projection(
        &fixture.kernel,
        &fixture.qdrant,
        &fixture.port,
        &fixture.card,
    )
    .republish_observed(
        &fixture.kernel.chunk_set,
        rebuild_guard(fixture.old.generation),
        None,
        &mut |_| ControlFlow::Continue(()),
    )
    .await
    .unwrap_err();

    assert!(
        matches!(&error, Error::UnrelatedAlias { alias, found }
            if *alias == fixture.old.alias && *found == stray),
        "{error}"
    );
    assert_eq!(
        state_of(&fixture.kernel, fixture.old.generation),
        GenerationState::Published
    );
}
