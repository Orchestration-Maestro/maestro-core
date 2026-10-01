//! A batch whose vectors break a check is refused whole: a vector of the
//! wrong dimension, a value that is not finite, a zero vector, or fewer
//! vectors than inputs. The batches before it stay written, the generation
//! stays building, and the alias does not move.

use super::{
    backends::{Backend, backends},
    kernel::Kernel,
    models::{Embedder, Fault, embedder},
    support::{alias_of, collection_of, projection, state_of},
};
use maestro_kernel::{gateway::ModelCard, generation::GenerationState};
use maestro_knowledge::index::{Error, Failure, Refusal};
use std::num::NonZeroUsize;

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
                inputs: 1,
                vectors: 0,
            },
        ),
    ];
    let kernel = Kernel::with_guides(1);
    let card = embedder(8);
    for backend in backends("a_batch_whose_vectors_break_a_check_is_refused_whole") {
        for (fault, refusal) in faults {
            refused_whole(&kernel, &backend, fault, refusal, &card).await;
        }
        backend.cleanup(&kernel.collection, 1..=1).await;
    }
}

/// One guide gives four chunks. At test batch size 3, the second batch goes
/// wrong as `fault` says. Reuse the kernel: each refusal leaves generation 1
/// building so the next call checks the same failure against the same corpus.
async fn refused_whole(
    kernel: &Kernel,
    backend: &Backend,
    fault: Fault,
    refusal: Refusal,
    card: &ModelCard,
) {
    let port = Embedder::default();
    port.fail(1, fault);
    let qdrant = backend.client();
    let projection =
        projection(kernel, &qdrant, &port, card).with_batch_size(NonZeroUsize::new(3).unwrap());
    let error = projection.publish(&kernel.chunk_set).await.unwrap_err();
    assert!(
        matches!(
            &error,
            Error::Embedding { at: 3, failure: Failure::Refused(found) } if *found == refusal
        ),
        "{} {fault:?}: {error}",
        backend.name
    );
    assert_eq!(
        backend.count(&collection_of(kernel, 1)).await,
        3,
        "{fault:?}"
    );
    assert_eq!(backend.alias(&alias_of(kernel)).await, None, "{fault:?}");
    assert_eq!(state_of(kernel, 1), GenerationState::Building);
    let published = kernel
        .database
        .published_generation(&kernel.scopes, &kernel.collection);
    assert_eq!(published.unwrap(), None);
}
