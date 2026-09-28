//! Refusals for malformed journaled projection rebuild state.

use super::super::backends::fake;
use super::rebuild_tests::{ResumeFixture, interrupt_rebuild, rebuild_guard, resume_fixture};
use maestro_kernel::{chunk_set::NewChunkSet, document::Collection, generation::NewGeneration};
use maestro_knowledge::index::{Error, Progress, RebuildGuard, Report};
use std::{collections::BTreeMap, ops::ControlFlow};

#[tokio::test]
async fn again_refuses_journaled_progress_with_wrong_chunk_counts_or_an_overrun() {
    let fixture = resume_fixture(fake()).await;
    let partial = interrupt_rebuild(&fixture).await;
    let expected_chunks = u64::try_from(fixture.chunks).unwrap();

    for invalid in [
        Progress {
            chunks: expected_chunks + 1,
            ..partial.clone()
        },
        Progress {
            indexed: expected_chunks + 1,
            ..partial.clone()
        },
    ] {
        let error = super::super::support::projection(
            &fixture.kernel,
            &fixture.qdrant,
            &fixture.port,
            &fixture.card,
        )
        .republish_observed(
            &fixture.kernel.chunk_set,
            rebuild_guard(fixture.old.generation),
            Some(&invalid),
            &mut |_| ControlFlow::Continue(()),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, Error::RecoveryTarget { .. }), "{error}");
    }
}

#[tokio::test]
async fn again_ignores_unfinished_targets_outside_the_frozen_tuple() {
    let fixture = resume_fixture(fake()).await;
    let other_set = "other-set";
    complete_empty_set(&fixture, &fixture.kernel.collection, other_set);

    let bad_targets = [
        NewGeneration {
            collection_id: fixture.kernel.collection.clone(),
            chunk_set_id: other_set.to_owned(),
            embedding_profile: fixture.old.embedding_profile.clone(),
            sparse_profile: fixture.old.sparse_profile.clone(),
        },
        NewGeneration {
            collection_id: fixture.kernel.collection.clone(),
            chunk_set_id: fixture.kernel.chunk_set.clone(),
            embedding_profile: "dense/other".to_owned(),
            sparse_profile: fixture.old.sparse_profile.clone(),
        },
        NewGeneration {
            collection_id: fixture.kernel.collection.clone(),
            chunk_set_id: fixture.kernel.chunk_set.clone(),
            embedding_profile: fixture.old.embedding_profile.clone(),
            sparse_profile: "sparse/other".to_owned(),
        },
    ];
    let mut watermark = fixture.old.generation;
    for bad_target in bad_targets {
        let bad = fixture
            .kernel
            .database
            .create_generation(&bad_target)
            .unwrap();
        let guard_watermark = bad.id;
        let mut observed = None;
        let error = super::super::support::projection(
            &fixture.kernel,
            &fixture.qdrant,
            &fixture.port,
            &fixture.card,
        )
        .republish_observed(
            &fixture.kernel.chunk_set,
            RebuildGuard {
                expected_published: Some(fixture.old.generation),
                generation_watermark: watermark,
            },
            None,
            &mut |progress| {
                observed = Some(progress.generation);
                ControlFlow::Break(())
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(error, Error::Stopped), "{error}");
        assert_ne!(observed, Some(bad.id));
        watermark = observed.unwrap();
        assert!(watermark > guard_watermark);
    }
}

#[tokio::test]
async fn again_finds_a_unique_unfinished_target_without_journaled_progress() {
    let fixture = resume_fixture(fake()).await;
    let partial = interrupt_rebuild(&fixture).await;

    let resumed = super::super::support::projection(
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
    .expect("find the unique unfinished target");

    assert_eq!(resumed.generation, partial.generation);
}

#[tokio::test]
async fn again_resumes_an_existing_collection_from_the_journaled_batch() {
    let fixture = resume_fixture(fake()).await;
    let partial = interrupt_rebuild(&fixture).await;
    let calls_before_resume = fixture.port.calls().len();

    let rebuilt = super::super::support::projection(
        &fixture.kernel,
        &fixture.qdrant,
        &fixture.port,
        &fixture.card,
    )
    .republish_observed(
        &fixture.kernel.chunk_set,
        rebuild_guard(fixture.old.generation),
        Some(&partial),
        &mut |_| ControlFlow::Continue(()),
    )
    .await
    .unwrap();

    assert_eq!(rebuilt.generation, partial.generation);
    let resumed_calls = &fixture.port.calls()[calls_before_resume..];
    assert_eq!(
        resumed_calls.iter().map(Vec::len).sum::<usize>(),
        fixture.chunks - usize::try_from(partial.indexed).unwrap()
    );
}

#[tokio::test]
async fn again_refuses_a_verified_target_with_the_wrong_search_profile() {
    let fixture = resume_fixture(fake()).await;
    let generation = fixture
        .kernel
        .database
        .create_generation(&NewGeneration {
            collection_id: fixture.kernel.collection.clone(),
            chunk_set_id: fixture.kernel.chunk_set.clone(),
            embedding_profile: fixture.old.embedding_profile.clone(),
            sparse_profile: fixture.old.sparse_profile.clone(),
        })
        .unwrap();
    fixture
        .kernel
        .database
        .begin_generation_search(&fixture.kernel.scopes, generation.id, "identifiers/other")
        .unwrap();
    fixture
        .kernel
        .database
        .complete_generation_search(&fixture.kernel.scopes, generation.id)
        .unwrap();
    fixture
        .kernel
        .database
        .verify_generation(generation.id, 0)
        .unwrap();

    let error = super::super::support::projection(
        &fixture.kernel,
        &fixture.qdrant,
        &fixture.port,
        &fixture.card,
    )
    .republish_observed(
        &fixture.kernel.chunk_set,
        rebuild_guard(fixture.old.generation),
        Some(&Progress {
            generation: generation.id,
            indexed: 0,
            chunks: u64::try_from(fixture.chunks).unwrap(),
            average_length: 0.0,
        }),
        &mut |_| ControlFlow::Continue(()),
    )
    .await
    .unwrap_err();
    assert!(matches!(error, Error::RecoveryTarget { .. }), "{error}");
}

#[tokio::test]
async fn again_refuses_a_resumed_target_with_a_different_embedding_or_sparse_profile() {
    let fixture = resume_fixture(fake()).await;
    for (embedding_profile, sparse_profile) in [
        ("dense/other", fixture.old.sparse_profile.as_str()),
        (fixture.old.embedding_profile.as_str(), "sparse/other"),
    ] {
        let generation = fixture
            .kernel
            .database
            .create_generation(&NewGeneration {
                collection_id: fixture.kernel.collection.clone(),
                chunk_set_id: fixture.kernel.chunk_set.clone(),
                embedding_profile: embedding_profile.to_owned(),
                sparse_profile: sparse_profile.to_owned(),
            })
            .unwrap();
        let error = super::super::support::projection(
            &fixture.kernel,
            &fixture.qdrant,
            &fixture.port,
            &fixture.card,
        )
        .republish_observed(
            &fixture.kernel.chunk_set,
            rebuild_guard(fixture.old.generation),
            Some(&Progress {
                generation: generation.id,
                indexed: 0,
                chunks: u64::try_from(fixture.chunks).unwrap(),
                average_length: 0.0,
            }),
            &mut |_| ControlFlow::Continue(()),
        )
        .await
        .unwrap_err();
        assert!(matches!(error, Error::RecoveryTarget { .. }), "{error}");
    }
}

#[tokio::test]
async fn again_refuses_a_journaled_target_at_or_below_the_frozen_watermark() {
    let fixture = resume_fixture(fake()).await;
    let partial = interrupt_rebuild(&fixture).await;

    let error = super::super::support::projection(
        &fixture.kernel,
        &fixture.qdrant,
        &fixture.port,
        &fixture.card,
    )
    .republish_observed(
        &fixture.kernel.chunk_set,
        RebuildGuard {
            expected_published: Some(fixture.old.generation),
            generation_watermark: partial.generation,
        },
        Some(&partial),
        &mut |_| ControlFlow::Continue(()),
    )
    .await
    .unwrap_err();
    assert!(
        matches!(error, Error::RecoveryTarget { generation } if generation == partial.generation),
        "{error}"
    );
}

#[tokio::test]
async fn again_refuses_a_resumed_target_of_another_chunk_set() {
    let fixture = resume_fixture(fake()).await;
    let other_set = "other-set";
    complete_empty_set(&fixture, &fixture.kernel.collection, other_set);
    let generation = fixture
        .kernel
        .database
        .create_generation(&NewGeneration {
            collection_id: fixture.kernel.collection.clone(),
            chunk_set_id: other_set.to_owned(),
            embedding_profile: fixture.old.embedding_profile.clone(),
            sparse_profile: fixture.old.sparse_profile.clone(),
        })
        .unwrap();

    let error = resume_at(
        &fixture,
        rebuild_guard(fixture.old.generation),
        generation.id,
    )
    .await
    .unwrap_err();
    assert!(
        matches!(error, Error::RecoveryTarget { generation: id } if id == generation.id),
        "{error}"
    );
}

#[tokio::test]
async fn again_reports_a_stale_guard_when_resuming_another_collections_publication() {
    let fixture = resume_fixture(fake()).await;
    let other_collection = format!("other-{}", fixture.kernel.collection);
    fixture
        .kernel
        .database
        .record_collection(&Collection {
            id: other_collection.clone(),
            title: "Other guides".to_owned(),
            visibility: "private".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    let other_set = "other-set";
    complete_empty_set(&fixture, &other_collection, other_set);
    let foreign = fixture
        .kernel
        .database
        .create_generation(&NewGeneration {
            collection_id: other_collection,
            chunk_set_id: other_set.to_owned(),
            embedding_profile: fixture.old.embedding_profile.clone(),
            sparse_profile: fixture.old.sparse_profile.clone(),
        })
        .unwrap();
    fixture
        .kernel
        .database
        .verify_generation(foreign.id, 0)
        .unwrap();
    fixture
        .kernel
        .database
        .publish_generation(foreign.id)
        .unwrap();

    let stale = RebuildGuard {
        expected_published: None,
        generation_watermark: fixture.old.generation,
    };
    let error = resume_at(&fixture, stale, foreign.id).await.unwrap_err();
    assert!(
        matches!(
            error,
            Error::PublishedChanged { expected: None, found: Some(found) }
                if found == fixture.old.generation
        ),
        "{error}"
    );
}

/// Records and completes the empty chunk set `id` of `collection`.
fn complete_empty_set(fixture: &ResumeFixture, collection: &str, id: &str) {
    let database = &fixture.kernel.database;
    database
        .begin_chunk_set(&NewChunkSet {
            id,
            collection_id: collection,
            chunk_profile: "synthetic/1",
            counter_contract_id: "synthetic-counter/1",
        })
        .unwrap();
    let manifest = database.put(b"{}", "application/json").unwrap();
    database.complete_chunk_set(id, &manifest).unwrap();
}

/// Resumes the fixture's rebuild from zero on the journaled `generation`.
async fn resume_at(
    fixture: &ResumeFixture,
    guard: RebuildGuard,
    generation: i64,
) -> Result<Report, Error> {
    super::super::support::projection(
        &fixture.kernel,
        &fixture.qdrant,
        &fixture.port,
        &fixture.card,
    )
    .republish_observed(
        &fixture.kernel.chunk_set,
        guard,
        Some(&Progress {
            generation,
            indexed: 0,
            chunks: u64::try_from(fixture.chunks).unwrap(),
            average_length: 0.0,
        }),
        &mut |_| ControlFlow::Continue(()),
    )
    .await
}
