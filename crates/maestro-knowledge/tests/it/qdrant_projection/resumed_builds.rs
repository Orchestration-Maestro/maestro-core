//! An interrupted build resumes: a job journals each batch as a step (T016),
//! and a rerun given the last step writes only the chunks after it; an
//! embedder without free room leaves the generation building for a rerun; a
//! step of another generation resumes nothing; and a generation left
//! verified is checked again and published without embedding anything.

use super::{
    backends::{Backend, backends, fake},
    kernel::Kernel,
    models::{Embedder, Fault, embedder},
    support::{alias_of, collection_of, projection, publish, state_of},
};
use maestro_kernel::{
    gateway,
    generation::GenerationState,
    job::{JobState, NewJob},
    scope::Scope,
};
use maestro_knowledge::index::{Error, Failure, Progress};
use serde_json::json;
use std::{
    ops::ControlFlow,
    time::{Duration, SystemTime},
};

/// How long a lease lasts in these tests.
const TERM: Duration = Duration::from_secs(60);

#[tokio::test]
async fn an_interrupted_build_resumes_at_its_last_journaled_batch() {
    for backend in backends("an_interrupted_build_resumes_at_its_last_journaled_batch") {
        resumes_at_its_last_journaled_batch(&backend).await;
    }
}

/// 50 guides give 151 chunks: batches of 64, 64 and 23. The first run
/// journals its first batch, then stops, as a job whose lease is lost does;
/// a rerun takes the lease over once it expired and resumes after it.
async fn resumes_at_its_last_journaled_batch(backend: &Backend) {
    let kernel = Kernel::with_guides(50);
    let (qdrant, card, port) = (backend.client(), embedder(8), Embedder::default());
    let publication = projection(&kernel, &qdrant, &port, &card);
    let scope: Scope = kernel.collection_scope().parse().unwrap();
    let inputs = json!({ "chunk_set": kernel.chunk_set });
    let new = NewJob {
        kind: "knowledge.publish",
        inputs: &inputs,
        scope: &scope,
        resource: Some("publication"),
    };
    let job = kernel.database.submit_job(&new, SystemTime::now()).unwrap();
    let mut lease = kernel
        .database
        .take_job(job.id, "first", SystemTime::now(), TERM)
        .unwrap();
    let stopped = publication
        .publish_observed(&kernel.chunk_set, None, &mut |progress: &Progress| {
            let step = serde_json::to_value(progress).unwrap();
            let now = SystemTime::now();
            kernel
                .database
                .progress(&mut lease, now, TERM, &step)
                .unwrap();
            ControlFlow::Break(())
        })
        .await
        .unwrap_err();
    assert!(matches!(stopped, Error::Stopped), "{stopped}");
    assert_eq!(backend.count(&collection_of(&kernel, 1)).await, 64);
    let later = SystemTime::now() + TERM * 2;
    let mut lease = kernel
        .database
        .take_job(job.id, "rerun", later, TERM)
        .unwrap();
    let last = kernel
        .database
        .last_progress(&kernel.scopes, job.id)
        .unwrap()
        .unwrap();
    let resume: Progress = serde_json::from_value(last.data).unwrap();
    assert_eq!(
        (resume.generation, resume.indexed, resume.chunks),
        (1, 64, 151)
    );
    let mut indexed = Vec::new();
    let report = publication
        .publish_observed(
            &kernel.chunk_set,
            Some(&resume),
            &mut |progress: &Progress| {
                let step = serde_json::to_value(progress).unwrap();
                kernel
                    .database
                    .progress(&mut lease, later, TERM, &step)
                    .unwrap();
                indexed.push(progress.indexed);
                ControlFlow::Continue(())
            },
        )
        .await
        .unwrap();
    let outcome = serde_json::to_value(&report).unwrap();
    kernel
        .database
        .complete_job(&lease, JobState::Succeeded, &outcome)
        .unwrap();
    assert_eq!(indexed, [128, 151]);
    let batches: Vec<usize> = port.calls().iter().map(Vec::len).collect();
    assert_eq!(batches, [64, 64, 23], "the rerun embeds from chunk 64 on");
    let chunks = kernel.chunks();
    let rerun: Vec<String> = chunks[64..]
        .iter()
        .map(|chunk| kernel.input(chunk))
        .collect();
    assert_eq!(port.calls()[1..].concat(), rerun);
    assert_eq!((report.generation, report.points), (1, 151));
    assert_eq!(backend.count(&report.qdrant_collection).await, 151);
    assert_eq!(state_of(&kernel, 1), GenerationState::Published);
}

#[tokio::test]
async fn an_embedder_without_free_room_leaves_the_generation_building() {
    for backend in backends("an_embedder_without_free_room_leaves_the_generation_building") {
        leaves_the_generation_building(&backend).await;
    }
}

/// 30 guides give 91 chunks, whose second batch finds no free room.
async fn leaves_the_generation_building(backend: &Backend) {
    let kernel = Kernel::with_guides(30);
    let (qdrant, card, port) = (backend.client(), embedder(8), Embedder::default());
    port.fail(1, Fault::Unavailable);
    let publication = projection(&kernel, &qdrant, &port, &card);
    let mut last = None;
    let error = publication
        .publish_observed(&kernel.chunk_set, None, &mut |progress: &Progress| {
            last = Some(progress.clone());
            ControlFlow::Continue(())
        })
        .await
        .unwrap_err();
    assert!(
        matches!(
            &error,
            Error::Embedding {
                at: 64,
                failure: Failure::Port(gateway::Error::Unavailable { .. })
            }
        ),
        "{error}"
    );
    assert_eq!(state_of(&kernel, 1), GenerationState::Building);
    assert_eq!(backend.count(&collection_of(&kernel, 1)).await, 64);
    assert_eq!(backend.alias(&alias_of(&kernel)).await, None);
    let report = publication
        .publish_observed(&kernel.chunk_set, last.as_ref(), &mut |_: &Progress| {
            ControlFlow::Continue(())
        })
        .await
        .unwrap();
    let batches: Vec<usize> = port.calls().iter().map(Vec::len).collect();
    assert_eq!(batches, [64, 27, 27]);
    assert_eq!((report.generation, report.points), (1, 91));
    assert_eq!(state_of(&kernel, 1), GenerationState::Published);
}

#[tokio::test]
async fn a_step_of_another_generation_resumes_nothing() {
    let backend = fake();
    let kernel = Kernel::with_guides(30);
    let (qdrant, card, port) = (backend.client(), embedder(8), Embedder::default());
    let publication = projection(&kernel, &qdrant, &port, &card);
    let stopped = publication
        .publish_observed(&kernel.chunk_set, None, &mut |_: &Progress| {
            ControlFlow::Break(())
        })
        .await
        .unwrap_err();
    assert!(matches!(stopped, Error::Stopped), "{stopped}");
    let elsewhere = Progress {
        generation: 7,
        indexed: 64,
        chunks: 91,
        average_length: 20.0,
    };
    let report = publication
        .publish_observed(&kernel.chunk_set, Some(&elsewhere), &mut |_: &Progress| {
            ControlFlow::Continue(())
        })
        .await
        .unwrap();
    let batches: Vec<usize> = port.calls().iter().map(Vec::len).collect();
    assert_eq!(batches, [64, 64, 27], "the rerun writes every chunk again");
    assert_eq!((report.generation, report.points), (1, 91));
}

#[tokio::test]
async fn a_generation_left_verified_is_checked_again_then_published() {
    for backend in backends("a_generation_left_verified_is_checked_again_then_published") {
        checked_again_then_published(&backend).await;
    }
}

/// A build stopped after its last batch is verified by hand, as a crash
/// between the check and the alias would leave it.
async fn checked_again_then_published(backend: &Backend) {
    let kernel = Kernel::with_guides(3);
    let (card, port) = (embedder(8), Embedder::default());
    let qdrant = backend.client();
    let stopped = projection(&kernel, &qdrant, &port, &card)
        .publish_observed(&kernel.chunk_set, None, &mut |_: &Progress| {
            ControlFlow::Break(())
        })
        .await
        .unwrap_err();
    assert!(matches!(stopped, Error::Stopped), "{stopped}");
    kernel.database.verify_generation(1, 10).unwrap();
    let calls = port.calls().len();
    let report = publish(&kernel, backend, &port, &card).await.unwrap();
    assert_eq!(
        port.calls().len(),
        calls,
        "a verified generation embeds nothing"
    );
    assert_eq!((report.generation, report.points), (1, 10));
    assert_eq!(state_of(&kernel, 1), GenerationState::Published);
    assert_eq!(
        backend.alias(&alias_of(&kernel)).await,
        Some(collection_of(&kernel, 1))
    );
}
