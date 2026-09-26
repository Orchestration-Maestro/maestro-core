//! An interrupted build resumes: a job journals each batch as a step (T016),
//! and a rerun given the last step writes only the chunks after it; an
//! embedder without free room leaves the generation building for a rerun; a
//! step of another generation resumes nothing; and a generation left
//! verified is checked again and published without embedding anything.

use super::{
    backends::{Backend, backends, fake},
    kernel::Kernel,
    models::{Embedder, Fault, embedder},
    support::{alias_of, collection_of, point_id, projection, publish, state_of},
};
use maestro_kernel::{
    gateway,
    generation::GenerationState,
    job::{Job, JobState, NewJob},
    scope::Scope,
};
use maestro_knowledge::{
    index::{Error, Failure, Progress, Projection, Report},
    lexical::{AverageLength, Passage},
};
use qdrant_client::qdrant::vector_output::Vector;
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
    let (average, passage_average) = whole_set_average(&kernel);
    let publication = projection(&kernel, &qdrant, &port, &card);
    let (job, resume) = interrupt_after_first_batch(&kernel, backend, &publication).await;
    assert_eq!(
        (resume.generation, resume.indexed, resume.chunks),
        (1, 64, 151)
    );
    let (report, journaled, indexed) =
        resume_and_complete(&kernel, &publication, job, resume).await;
    assert_eq!(indexed, [128, 151]);
    assert_journaled_average(&journaled, average);
    assert_last_batch_sparse(backend, &kernel, &report.qdrant_collection, passage_average).await;
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
    backend.cleanup(&kernel.collection, 1..=1).await;
}

async fn interrupt_after_first_batch(
    kernel: &Kernel,
    backend: &Backend,
    publication: &Projection<'_, Embedder>,
) -> (Job, Progress) {
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
            kernel
                .database
                .progress(&mut lease, SystemTime::now(), TERM, &step)
                .unwrap();
            ControlFlow::Break(())
        })
        .await
        .unwrap_err();
    assert!(matches!(stopped, Error::Stopped), "{stopped}");
    assert_eq!(backend.count(&collection_of(kernel, 1)).await, 64);
    let last = kernel
        .database
        .last_progress(&kernel.scopes, job.id)
        .unwrap()
        .unwrap();
    let resume = serde_json::from_value(last.data).unwrap();
    (job, resume)
}

async fn resume_and_complete(
    kernel: &Kernel,
    publication: &Projection<'_, Embedder>,
    job: Job,
    resume: Progress,
) -> (Report, Vec<Progress>, Vec<u64>) {
    let later = SystemTime::now() + TERM * 2;
    let mut lease = kernel
        .database
        .take_job(job.id, "rerun", later, TERM)
        .unwrap();
    let mut journaled = vec![resume.clone()];
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
                journaled.push(progress.clone());
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
    (report, journaled, indexed)
}

fn assert_journaled_average(journaled: &[Progress], average: f64) {
    assert!(
        journaled
            .iter()
            .all(|progress| (progress.average_length - average).abs() < 1e-12),
        "every journaled batch uses the whole set's average length"
    );
}

fn whole_set_average(kernel: &Kernel) -> (f64, AverageLength) {
    let chunks = kernel.chunks();
    let terms = chunks
        .iter()
        .map(|chunk| Passage::new(&kernel.input(chunk)).term_count())
        .sum::<usize>();
    let terms = f64::from(u32::try_from(terms).unwrap());
    let passages = f64::from(u32::try_from(chunks.len()).unwrap());
    let average = terms / passages;
    (average, AverageLength::new(average).unwrap())
}

async fn assert_last_batch_sparse(
    backend: &Backend,
    kernel: &Kernel,
    collection: &str,
    average: AverageLength,
) {
    let last = kernel.chunks().pop().unwrap();
    let expected = Passage::new(&kernel.input(&last)).vector(average);
    let point = backend.point(collection, point_id(&last.id)).await;
    let vectors = point.vectors.unwrap();
    let Some(Vector::Sparse(found)) = vectors.get_vector_by_name("bm25") else {
        panic!("{} {}: no sparse vector", backend.name, last.id);
    };
    assert_eq!(found.indices, expected.indices(), "{}", backend.name);
    assert_eq!(
        found.values.len(),
        expected.values().len(),
        "{}",
        backend.name
    );
    for (found, expected) in found.values.iter().zip(expected.values()) {
        assert!(
            (found - expected).abs() < 1e-6,
            "{} {}",
            backend.name,
            last.id
        );
    }
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
    backend.cleanup(&kernel.collection, 1..=1).await;
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
    backend.cleanup(&kernel.collection, 1..=1).await;
}

#[tokio::test]
async fn a_recreated_collection_starts_at_the_first_chunk() {
    let backend = fake();
    let kernel = Kernel::with_guides(30);
    let (qdrant, card, port) = (backend.client(), embedder(8), Embedder::default());
    let publication = projection(&kernel, &qdrant, &port, &card);
    let mut resume = None;
    let stopped = publication
        .publish_observed(&kernel.chunk_set, None, &mut |progress: &Progress| {
            resume = Some(progress.clone());
            ControlFlow::Break(())
        })
        .await
        .unwrap_err();
    assert!(matches!(stopped, Error::Stopped), "{stopped}");
    let collection = collection_of(&kernel, 1);
    backend.delete_collection(&collection).await;

    let report = publication
        .publish_observed(&kernel.chunk_set, resume.as_ref(), &mut |_: &Progress| {
            ControlFlow::Continue(())
        })
        .await
        .unwrap();
    assert_eq!((report.generation, report.points), (1, 91));
    assert_eq!(backend.count(&collection).await, 91);
    let batches: Vec<usize> = port.calls().iter().map(Vec::len).collect();
    assert_eq!(batches, [64, 64, 27]);
    backend.cleanup(&kernel.collection, 1..=1).await;
}

#[tokio::test]
async fn a_refused_alias_switch_leaves_a_verified_generation_for_rerun() {
    let backend = fake();
    let kernel = Kernel::with_guides(3);
    let port = Embedder::default();
    let first = publish(&kernel, &backend, &port, &embedder(8))
        .await
        .unwrap();
    let alias = alias_of(&kernel);
    backend.refuse_create_alias();
    let refused = publish(&kernel, &backend, &port, &embedder(6))
        .await
        .unwrap_err();
    assert!(matches!(refused, Error::Qdrant(_)), "{refused}");
    assert_eq!(state_of(&kernel, 2), GenerationState::Verified);
    assert_eq!(backend.alias(&alias).await, Some(collection_of(&kernel, 1)));

    let report = publish(&kernel, &backend, &port, &embedder(6))
        .await
        .unwrap();
    assert_eq!(
        (report.generation, report.retired),
        (2, Some(first.generation))
    );
    assert_eq!(state_of(&kernel, 1), GenerationState::Retired);
    assert_eq!(state_of(&kernel, 2), GenerationState::Published);
    assert_eq!(backend.alias(&alias).await, Some(collection_of(&kernel, 2)));
    backend.cleanup(&kernel.collection, 1..=2).await;
}
