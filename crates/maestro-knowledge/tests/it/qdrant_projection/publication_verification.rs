//! Published-generation verification names corrupted inputs, bad counts and
//! a misplaced alias.

use super::{
    backends::{Backend, fake},
    kernel::Kernel,
    models::{Embedder, embedder, v2_embedder},
    support::{alias_of, collection_of, point_id, projection, publish},
};
use maestro_kernel::generation::NewGeneration;
use maestro_knowledge::{
    index::{Error as ProjectionError, Progress, QdrantError, Unverified},
    lexical::PROFILE as SPARSE_PROFILE,
    publish::{Error, verify_generation},
};
use std::{error::Error as _, num::NonZeroUsize, ops::ControlFlow};

#[tokio::test]
async fn mismatched_v2_counter_stops_before_generation_model_or_qdrant_effects() {
    let backend = fake();
    let kernel = Kernel::with_guides(1);
    let port = Embedder::default();
    let card = v2_embedder();

    let result = publish(&kernel, &backend, &port, &card).await;

    assert!(matches!(
        result,
        Err(ProjectionError::CounterContractMismatch { .. })
    ));
    assert!(port.calls().is_empty());
    assert_eq!(backend.fake.as_ref().unwrap().collection_count(), 0);
    assert!(backend.fake.as_ref().unwrap().calls().is_empty());
    assert!(
        kernel
            .database
            .generations(&kernel.scopes, &kernel.collection)
            .unwrap()
            .is_empty()
    );
    assert!(
        kernel
            .database
            .published_generation(&kernel.scopes, &kernel.collection)
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn verification_names_missing_and_corrupt_artifacts_count_and_alias_findings() {
    let backend = fake();
    let kernel = Kernel::with_guides(1);
    let other = Kernel::with_guides(1);
    let (card, embedder) = (embedder(8), Embedder::default());
    let report = publish(&kernel, &backend, &embedder, &card).await.unwrap();
    let wrong = publish(&other, &backend, &embedder, &card).await.unwrap();
    let qdrant = backend.client();
    let verification =
        verify_generation(&kernel.database, &kernel.scopes, &qdrant, report.generation)
            .await
            .unwrap();
    assert!(
        verification.findings.is_empty(),
        "{:#?}",
        verification.findings
    );
    let chunks = kernel.chunks();
    kernel.remove_artifact(&chunks[0].digest);
    kernel.corrupt_artifact(&chunks[1].digest, b"not the prepared input");
    backend.intrude(&report.qdrant_collection, 8).await;
    backend
        .fake
        .as_ref()
        .unwrap()
        .alias_to(&alias_of(&kernel), &wrong.qdrant_collection);

    let verification =
        verify_generation(&kernel.database, &kernel.scopes, &qdrant, report.generation)
            .await
            .unwrap();
    assert!(
        verification
            .findings
            .iter()
            .any(|finding| finding.contains("missing artifact")),
        "{:#?}",
        verification.findings
    );
    assert!(
        verification
            .findings
            .iter()
            .any(|finding| finding.contains("digest mismatch"))
    );
    assert!(
        verification
            .findings
            .iter()
            .any(|finding| finding.contains("point count mismatch"))
    );
    assert!(
        verification
            .findings
            .iter()
            .any(|finding| finding.contains("alias") && finding.contains("names"))
    );
    assert!(
        verification
            .findings
            .iter()
            .any(|finding| finding.contains(&wrong.qdrant_collection))
    );
    assert_eq!(
        collection_of(&kernel, report.generation),
        report.qdrant_collection
    );
}

#[tokio::test]
async fn publication_does_not_reuse_a_generation_with_another_search_profile() {
    let backend = fake();
    let kernel = Kernel::with_guides(1);
    let card = embedder(8);
    let embedding_profile = format!("dense/1:sha256:{}", card.digest().as_str());
    let existing = kernel
        .database
        .create_generation(&NewGeneration {
            collection_id: kernel.collection.clone(),
            chunk_set_id: kernel.chunk_set.clone(),
            embedding_profile,
            sparse_profile: "other-sparse-profile".to_owned(),
        })
        .unwrap();
    let report = publish(&kernel, &backend, &Embedder::default(), &card)
        .await
        .unwrap();

    assert_ne!(report.generation, existing.id);
    assert_eq!(report.generation, existing.id + 1);
    backend
        .cleanup(&kernel.collection, [existing.id, report.generation])
        .await;
}

#[tokio::test]
async fn publication_does_not_reuse_a_ready_generation_with_another_identifier_profile() {
    let backend = fake();
    let kernel = Kernel::with_guides(1);
    let card = embedder(8);
    let existing = kernel
        .database
        .create_generation(&NewGeneration {
            collection_id: kernel.collection.clone(),
            chunk_set_id: kernel.chunk_set.clone(),
            embedding_profile: format!("dense/1:sha256:{}", card.digest().as_str()),
            sparse_profile: SPARSE_PROFILE.to_owned(),
        })
        .unwrap();
    assert!(
        kernel
            .database
            .begin_generation_search(&kernel.scopes, existing.id, "other-identifier-profile")
            .unwrap()
    );
    kernel
        .database
        .complete_generation_search(&kernel.scopes, existing.id)
        .unwrap();
    let point_count = u64::try_from(kernel.chunks().len()).unwrap();
    kernel
        .database
        .verify_generation(existing.id, point_count)
        .unwrap();
    kernel.database.publish_generation(existing.id).unwrap();

    let report = publish(&kernel, &backend, &Embedder::default(), &card)
        .await
        .unwrap();
    assert_ne!(report.generation, existing.id);
    assert_eq!(report.generation, existing.id + 1);
    backend
        .cleanup(&kernel.collection, [existing.id, report.generation])
        .await;
}

#[tokio::test]
async fn publication_refuses_one_mismatched_payload_ownership_field() {
    let backend = fake();
    let kernel = Kernel::with_guides(1);
    let (card, port) = (embedder(8), Embedder::default());
    let qdrant = backend.client();
    let projection = projection(&kernel, &qdrant, &port, &card);
    let mut changed = false;
    let result = projection
        .publish_observed(&kernel.chunk_set, None, &mut |progress: &Progress| {
            if !changed {
                backend.fake.as_ref().unwrap().set_payload_text(
                    &collection_of(&kernel, progress.generation),
                    point_id("chunk-0-lead"),
                    "revision_id",
                    "foreign-revision",
                );
                changed = true;
            }
            ControlFlow::Continue(())
        })
        .await;

    assert!(matches!(
        result,
        Err(ProjectionError::Unverified {
            reason: Unverified::Search { reason },
            ..
        }) if reason == "a published search point has inconsistent chunk ownership"
    ));
    assert!(changed);
}

#[test]
fn verification_errors_keep_their_message_and_cause() {
    let error = Error::Qdrant(QdrantError::InvalidAnswer("no count".to_owned()));
    assert_eq!(
        error.to_string(),
        "Qdrant failed: Qdrant's answer is not what was asked for: no count"
    );
    assert!(error.source().is_some());
}

#[tokio::test]
async fn qdrant_returns_the_target_of_the_named_alias_when_others_exist() {
    let backend = fake();
    let first = Kernel::with_guides(1);
    let second = Kernel::with_guides(1);
    let (card, embedder) = (embedder(8), Embedder::default());
    let report = publish(&first, &backend, &embedder, &card).await.unwrap();
    publish(&second, &backend, &embedder, &card).await.unwrap();
    let qdrant = backend.client();
    let actual = qdrant.alias_collection(&alias_of(&first)).await.unwrap();
    assert_eq!(actual.as_deref(), Some(report.qdrant_collection.as_str()));
}

/// Builds the first batch of `guides`, leaving its generation building.
async fn stopped_after_first_batch(
    guides: usize,
    batch_size: Option<NonZeroUsize>,
) -> (Backend, Kernel, Progress) {
    let backend = fake();
    let kernel = Kernel::with_guides(guides);
    let (card, embedder) = (embedder(8), Embedder::default());
    let qdrant = backend.client();
    let projection = projection(&kernel, &qdrant, &embedder, &card);
    let mut progress = None;
    let mut stop_after_batch = |step: &Progress| {
        progress = Some(step.clone());
        ControlFlow::Break(())
    };
    let stopped = if let Some(batch_size) = batch_size {
        projection
            .with_batch_size(batch_size)
            .publish_observed(&kernel.chunk_set, None, &mut stop_after_batch)
            .await
    } else {
        projection
            .publish_observed(&kernel.chunk_set, None, &mut stop_after_batch)
            .await
    };
    assert!(matches!(stopped, Err(ProjectionError::Stopped)));
    (backend, kernel, progress.unwrap())
}

#[tokio::test]
async fn generation_count_mismatch_is_reported_when_qdrant_and_chunks_agree() {
    let (backend, kernel, step) = stopped_after_first_batch(1, None).await;
    assert_eq!(step.indexed, step.chunks);
    kernel
        .database
        .verify_generation(step.generation, step.indexed + 1)
        .unwrap();
    kernel.database.publish_generation(step.generation).unwrap();
    let qdrant = backend.client();
    let verification =
        verify_generation(&kernel.database, &kernel.scopes, &qdrant, step.generation)
            .await
            .unwrap();
    assert!(
        verification
            .findings
            .iter()
            .any(|finding| finding.contains("point count mismatch")),
        "{:#?}",
        verification.findings
    );
}

#[tokio::test]
async fn qdrant_count_mismatch_is_reported_when_the_generation_agrees_with_qdrant() {
    let (backend, kernel, step) =
        stopped_after_first_batch(1, Some(NonZeroUsize::new(3).unwrap())).await;
    assert_eq!(step.indexed, 3);
    assert!(step.indexed < step.chunks);
    kernel
        .database
        .verify_generation(step.generation, step.indexed)
        .unwrap();
    kernel.database.publish_generation(step.generation).unwrap();
    let qdrant = backend.client();
    let verification =
        verify_generation(&kernel.database, &kernel.scopes, &qdrant, step.generation)
            .await
            .unwrap();
    assert!(
        verification
            .findings
            .iter()
            .any(|finding| finding.contains("point count mismatch")),
        "{:#?}",
        verification.findings
    );
}
