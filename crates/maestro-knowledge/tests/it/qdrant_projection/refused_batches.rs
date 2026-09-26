//! A batch whose vectors break a check is refused whole: a vector of the
//! wrong dimension, a value that is not finite, a zero vector, or fewer
//! vectors than inputs. The batches before it stay written, the generation
//! stays building, and the alias does not move.

use super::{
    backends::{Backend, backends},
    kernel::Kernel,
    models::{Embedder, Fault, embedder},
    support::{alias_of, collection_of, publish, state_of},
};
use maestro_kernel::generation::GenerationState;
use maestro_knowledge::index::{Error, Failure, Refusal};

#[tokio::test]
async fn a_batch_whose_vectors_break_a_check_is_refused_whole() {
    let faults = [
        (
            Fault::Dimensions,
            Refusal::Dimensions {
                input: 0,
                expected: 8,
                found: 7,
            },
        ),
        (Fault::NotANumber, Refusal::NonFinite { input: 0 }),
        (Fault::Infinite, Refusal::NonFinite { input: 0 }),
        (Fault::Zero, Refusal::Zero { input: 0 }),
        (
            Fault::Fewer,
            Refusal::Count {
                inputs: 27,
                vectors: 26,
            },
        ),
    ];
    for backend in backends("a_batch_whose_vectors_break_a_check_is_refused_whole") {
        for (fault, refusal) in faults {
            refused_whole(&backend, fault, refusal).await;
        }
    }
}

/// 30 guides give 91 chunks, whose second batch, from chunk 64, goes wrong
/// as `fault` says.
async fn refused_whole(backend: &Backend, fault: Fault, refusal: Refusal) {
    let kernel = Kernel::with_guides(30);
    let port = Embedder::default();
    port.fail(1, fault);
    let error = publish(&kernel, backend, &port, &embedder(8))
        .await
        .unwrap_err();
    assert!(
        matches!(
            &error,
            Error::Embedding { at: 64, failure: Failure::Refused(found) } if *found == refusal
        ),
        "{} {fault:?}: {error}",
        backend.name
    );
    assert_eq!(
        backend.count(&collection_of(&kernel, 1)).await,
        64,
        "{fault:?}"
    );
    assert_eq!(backend.alias(&alias_of(&kernel)).await, None, "{fault:?}");
    assert_eq!(state_of(&kernel, 1), GenerationState::Building);
    let published = kernel
        .database
        .published_generation(&kernel.scopes, &kernel.collection);
    assert_eq!(published.unwrap(), None);
}
