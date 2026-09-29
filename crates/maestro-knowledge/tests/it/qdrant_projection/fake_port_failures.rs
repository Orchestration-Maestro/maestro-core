//! Failure, resume, and rollback tests for the fake projection port.

use super::{
    fake_port::{FakeCollection, FakePort},
    kernel::Kernel,
    models::{Embedder, embedder},
    support::{alias_of, collection_of, state_of},
};
use maestro_kernel::{
    generation::GenerationState,
    job::{JobState, NewJob},
    scope::Scope,
};
use maestro_knowledge::index::{
    CollectionLayout, Error, Progress, Projection, QdrantError, RetrievalProjectionPort, Unverified,
};
use serde_json::json;
use std::ops::ControlFlow;
use std::time::{Duration, SystemTime};

const JOB_TERM: Duration = Duration::from_secs(60);

#[tokio::test]
async fn wrong_precreated_layout_fails_its_generation() {
    let kernel = Kernel::with_guides(1);
    let fake = FakePort::default();
    fake.collections.lock().unwrap().insert(
        collection_of(&kernel, 1),
        FakeCollection {
            layout: Some(CollectionLayout {
                dense_dimensions: 8,
                dense_present: true,
                dense_distance: "Euclid".to_owned(),
                sparse_present: true,
                sparse_modifier: Some("Idf".to_owned()),
            }),
            ..FakeCollection::default()
        },
    );
    let port = Embedder::default();
    let card = embedder(8);
    let error = Projection {
        database: &kernel.database,
        scopes: &kernel.scopes,
        projection: &fake,
        port: &port,
        card: &card,
    }
    .publish(&kernel.chunk_set)
    .await
    .unwrap_err();
    assert!(
        matches!(
            error,
            Error::Unverified {
                reason: Unverified::Vectors { .. },
                ..
            }
        ),
        "{error}"
    );
    assert_eq!(state_of(&kernel, 1), GenerationState::Failed);
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "The test journals the successful batch before refusal and resume."
)]
async fn upsert_refusal_keeps_building_generation_and_journaled_resume_finishes() {
    let kernel = Kernel::with_changed_guides(1, &|_, _, original| {
        let mut chunks = Vec::with_capacity(130);
        for index in 0..129 {
            let mut chunk = original[0].clone();
            chunk.id = format!("chunk-extra-{index}");
            chunks.push(chunk);
        }
        let mut last = original[0].clone();
        last.id = "chunk-49-2".to_owned();
        chunks.push(last);
        chunks
    });
    let fake = FakePort::default();
    fake.fail_after("upsert_points", 1);
    let port = Embedder::default();
    let card = embedder(8);
    let scope: Scope = kernel.collection_scope().parse().unwrap();
    let inputs = json!({ "chunk_set": kernel.chunk_set });
    let job = kernel
        .database
        .submit_job(
            &NewJob {
                kind: "knowledge.publish",
                inputs: &inputs,
                scope: &scope,
                resource: Some("publication"),
            },
            SystemTime::now(),
        )
        .unwrap();
    let now = SystemTime::now();
    let mut lease = kernel
        .database
        .take_job(job.id, "first", now, JOB_TERM)
        .unwrap();
    let publication = Projection {
        database: &kernel.database,
        scopes: &kernel.scopes,
        projection: &fake,
        port: &port,
        card: &card,
    };
    let mut recorded = None;
    let error = publication
        .publish_observed(&kernel.chunk_set, None, &mut |step| {
            kernel
                .database
                .progress(
                    &mut lease,
                    now,
                    JOB_TERM,
                    &serde_json::to_value(step).unwrap(),
                )
                .unwrap();
            recorded = Some(step.clone());
            ControlFlow::Continue(())
        })
        .await
        .unwrap_err();
    assert!(
        matches!(error, Error::Qdrant(QdrantError::InvalidAnswer(_))),
        "{error}"
    );
    let generation = recorded.as_ref().unwrap().generation;
    assert_eq!(state_of(&kernel, generation), GenerationState::Building);
    let journal = kernel
        .database
        .last_progress(&kernel.scopes, job.id)
        .unwrap()
        .unwrap();
    let resume: Progress = serde_json::from_value(journal.data).unwrap();
    assert_eq!(resume.indexed, 64);

    let later = now + JOB_TERM * 2;
    let mut lease = kernel
        .database
        .take_job(job.id, "rerun", later, JOB_TERM)
        .unwrap();
    let report = publication
        .publish_observed(&kernel.chunk_set, Some(&resume), &mut |step| {
            kernel
                .database
                .progress(
                    &mut lease,
                    later,
                    JOB_TERM,
                    &serde_json::to_value(step).unwrap(),
                )
                .unwrap();
            ControlFlow::Continue(())
        })
        .await
        .unwrap();
    kernel
        .database
        .complete_job(
            &lease,
            JobState::Succeeded,
            &serde_json::to_value(&report).unwrap(),
        )
        .unwrap();
    assert_eq!(report.points, 130);
    assert_eq!(state_of(&kernel, generation), GenerationState::Published);
}

#[tokio::test]
async fn failed_verified_recheck_restores_the_previous_alias() {
    let kernel = Kernel::with_guides(3);
    let fake = FakePort::default();
    let port = Embedder::default();
    let original_card = embedder(8);
    let original = Projection {
        database: &kernel.database,
        scopes: &kernel.scopes,
        projection: &fake,
        port: &port,
        card: &original_card,
    }
    .publish(&kernel.chunk_set)
    .await
    .unwrap();
    let alias = alias_of(&kernel);
    assert_eq!(
        fake.alias_target(&alias).await.unwrap(),
        Some(original.qdrant_collection.clone())
    );

    let replacement_card = embedder(6);
    let replacement = Projection {
        database: &kernel.database,
        scopes: &kernel.scopes,
        projection: &fake,
        port: &port,
        card: &replacement_card,
    };
    let mut progress = None;
    let stopped = replacement
        .publish_observed(&kernel.chunk_set, None, &mut |step| {
            if step.indexed == step.chunks {
                kernel
                    .database
                    .complete_generation_search(&kernel.scopes, step.generation)
                    .unwrap();
                kernel
                    .database
                    .verify_generation(step.generation, step.chunks)
                    .unwrap();
                fake.aliases
                    .lock()
                    .unwrap()
                    .insert(alias.clone(), collection_of(&kernel, step.generation));
                progress = Some(step.clone());
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        })
        .await
        .unwrap_err();
    assert!(matches!(stopped, Error::Stopped));
    let candidate = progress.unwrap();
    let candidate_collection = collection_of(&kernel, candidate.generation);
    assert_eq!(
        fake.alias_target(&alias).await.unwrap(),
        Some(candidate_collection.clone())
    );
    let id = fake
        .collections
        .lock()
        .unwrap()
        .get(&candidate_collection)
        .unwrap()
        .points
        .keys()
        .next()
        .unwrap()
        .clone();
    fake.drop_point(&candidate_collection, &id);

    let error = replacement.publish(&kernel.chunk_set).await.unwrap_err();
    assert!(
        matches!(
            error,
            Error::Unverified {
                reason: Unverified::Count { .. } | Unverified::Missing { .. },
                ..
            }
        ),
        "{error}"
    );
    assert_eq!(
        state_of(&kernel, candidate.generation),
        GenerationState::Failed
    );
    assert_eq!(
        fake.alias_target(&alias).await.unwrap(),
        Some(original.qdrant_collection)
    );
}
