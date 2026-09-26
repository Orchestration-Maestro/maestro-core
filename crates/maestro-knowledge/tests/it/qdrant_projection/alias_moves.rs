//! The alias moves only to a generation whose collection passes its checks:
//! one that holds a point no chunk has, lacks the point of a chunk, or has
//! other vectors than its profiles call for fails for good, and the alias
//! stays where it was; the next generation that passes takes it, and retires
//! the one before, whose collection stays.

use super::{
    backends::{Backend, backends},
    kernel::Kernel,
    models::{Embedder, embedder},
    support::{alias_of, collection_of, point_id, projection, publish, state_of},
};
use maestro_kernel::generation::GenerationState;
use maestro_knowledge::index::{Error, Progress, Unverified};
use qdrant_client::qdrant::{Distance, Modifier};
use std::ops::ControlFlow;

#[tokio::test]
async fn the_alias_stays_while_a_generation_fails_its_check() {
    for backend in backends("the_alias_stays_while_a_generation_fails_its_check") {
        stays_while_a_generation_fails(&backend).await;
    }
}

/// The first generation takes the alias; the second, of another embedder,
/// finds its collection holding a point no chunk has; the third passes.
async fn stays_while_a_generation_fails(backend: &Backend) {
    let kernel = Kernel::with_guides(3);
    let port = Embedder::default();
    let first = publish(&kernel, backend, &port, &embedder(8))
        .await
        .unwrap();
    let alias = alias_of(&kernel);
    assert_eq!(backend.alias(&alias).await, Some(collection_of(&kernel, 1)));
    second_fails_its_check(backend, &kernel, &port).await;
    let third = publish(&kernel, backend, &port, &embedder(6))
        .await
        .unwrap();
    assert_eq!(
        (third.generation, third.retired),
        (3, Some(first.generation))
    );
    assert_eq!(backend.alias(&alias).await, Some(collection_of(&kernel, 3)));
    assert_eq!(state_of(&kernel, 1), GenerationState::Retired);
    assert_eq!(state_of(&kernel, 3), GenerationState::Published);
    assert!(
        backend.exists(&first.qdrant_collection).await,
        "kept for a rollback"
    );
}

/// The second generation of `kernel`, of an embedder of 6 dimensions, finds
/// its collection holding a point no chunk has: it fails, and the alias
/// stays with the first.
async fn second_fails_its_check(backend: &Backend, kernel: &Kernel, port: &Embedder) {
    let second = collection_of(kernel, 2);
    backend
        .create(&second, 6, Distance::Cosine, Some(Modifier::Idf))
        .await;
    backend.intrude(&second, 6).await;
    let error = publish(kernel, backend, port, &embedder(6))
        .await
        .unwrap_err();
    let count = Unverified::Count {
        expected: 10,
        found: 11,
    };
    assert!(
        matches!(&error, Error::Unverified { generation: 2, reason } if *reason == count),
        "{} {error}",
        backend.name
    );
    let alias = backend.alias(&alias_of(kernel)).await;
    assert_eq!(alias, Some(collection_of(kernel, 1)));
    assert_eq!(state_of(kernel, 2), GenerationState::Failed);
    assert_eq!(state_of(kernel, 1), GenerationState::Published);
}

#[tokio::test]
async fn a_generation_lacking_the_point_of_a_chunk_fails_its_check() {
    for backend in backends("a_generation_lacking_the_point_of_a_chunk_fails_its_check") {
        lacking_a_point_fails(&backend).await;
    }
}

/// A build stopped after its only batch loses the point of a chunk and gains
/// one no chunk has, so its count still matches; the rerun, which has no
/// batch left, finds the point missing.
async fn lacking_a_point_fails(backend: &Backend) {
    let kernel = Kernel::with_guides(3);
    let (qdrant, card, port) = (backend.client(), embedder(8), Embedder::default());
    let publication = projection(&kernel, &qdrant, &port, &card);
    let mut last = None;
    let stopped = publication
        .publish_observed(&kernel.chunk_set, None, &mut |progress: &Progress| {
            last = Some(progress.clone());
            ControlFlow::Break(())
        })
        .await
        .unwrap_err();
    assert!(matches!(stopped, Error::Stopped), "{stopped}");
    let collection = collection_of(&kernel, 1);
    backend.delete(&collection, point_id("chunk-0-1")).await;
    backend.intrude(&collection, 8).await;
    let error = publication
        .publish_observed(&kernel.chunk_set, last.as_ref(), &mut |_: &Progress| {
            ControlFlow::Continue(())
        })
        .await
        .unwrap_err();
    let missing = Unverified::Missing {
        chunk: "chunk-0-1".to_owned(),
    };
    assert!(
        matches!(&error, Error::Unverified { generation: 1, reason } if *reason == missing),
        "{} {error}",
        backend.name
    );
    assert_eq!(state_of(&kernel, 1), GenerationState::Failed);
    assert_eq!(backend.alias(&alias_of(&kernel)).await, None);
}

#[tokio::test]
async fn a_collection_with_other_vectors_fails_its_generation_before_any_point() {
    for backend in backends("a_collection_with_other_vectors_fails_its_generation_before_any_point")
    {
        for (dimensions, distance, modifier) in [
            (5, Distance::Cosine, Some(Modifier::Idf)),
            (8, Distance::Dot, Some(Modifier::Idf)),
            (8, Distance::Cosine, None),
        ] {
            other_vectors_fail(&backend, dimensions, distance, modifier).await;
        }
    }
}

/// The first generation's collection exists already, with a dense vector of
/// `dimensions` compared by `distance`, and a sparse vector weighted by
/// `modifier`, if any.
async fn other_vectors_fail(
    backend: &Backend,
    dimensions: u64,
    distance: Distance,
    modifier: Option<Modifier>,
) {
    let kernel = Kernel::with_guides(3);
    let collection = collection_of(&kernel, 1);
    backend
        .create(&collection, dimensions, distance, modifier)
        .await;
    let port = Embedder::default();
    let error = publish(&kernel, backend, &port, &embedder(8))
        .await
        .unwrap_err();
    assert!(
        matches!(
            &error,
            Error::Unverified {
                generation: 1,
                reason: Unverified::Vectors { dimensions: 8, .. }
            }
        ),
        "{} {dimensions} {distance:?} {modifier:?}: {error}",
        backend.name
    );
    assert_eq!(state_of(&kernel, 1), GenerationState::Failed);
    assert!(port.calls().is_empty(), "nothing is embedded");
    assert_eq!(backend.count(&collection).await, 0);
}
