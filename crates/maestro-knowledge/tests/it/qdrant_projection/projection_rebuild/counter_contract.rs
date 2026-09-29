//! Recovery matches the v2 counter identity exactly.

use super::super::{
    backends::fake,
    kernel::Kernel,
    models::{Embedder, v2_embedder, v2_embedder_with_query_prefix},
    support::{projection, publish},
};
use maestro_knowledge::index::{Error, RebuildGuard};
use std::ops::ControlFlow;

#[tokio::test]
async fn matching_v2_recovery_succeeds_and_same_dimension_other_card_is_refused() {
    let backend = fake();
    let card_a = v2_embedder();
    let card_a_counter = format!("router/1:sha256:{}", card_a.digest().as_str());
    let kernel = Kernel::with_counter_contract(1, &card_a_counter);
    let port_a = Embedder::default();
    let old = publish(&kernel, &backend, &port_a, &card_a).await.unwrap();
    backend.delete_collection(&old.qdrant_collection).await;

    let rebuilt = projection(&kernel, &backend.client(), &port_a, &card_a)
        .republish_observed(
            &kernel.chunk_set,
            rebuild_guard(old.generation),
            None,
            &mut |_| ControlFlow::Continue(()),
        )
        .await
        .unwrap();
    assert_ne!(rebuilt.generation, old.generation);
    assert_eq!(backend.count(&rebuilt.qdrant_collection).await, old.points);

    let card_b = v2_embedder_with_query_prefix();
    assert_eq!(card_a.fields().dimensions, card_b.fields().dimensions);
    assert_ne!(card_a.digest(), card_b.digest());
    backend.delete_collection(&rebuilt.qdrant_collection).await;
    let backend_calls = backend.fake.as_ref().unwrap().calls();
    let generation_ids_before = generation_ids(&kernel);
    let port_b = Embedder::default();
    let error = projection(&kernel, &backend.client(), &port_b, &card_b)
        .republish_observed(
            &kernel.chunk_set,
            rebuild_guard(rebuilt.generation),
            None,
            &mut |_| ControlFlow::Continue(()),
        )
        .await
        .unwrap_err();

    let Error::CounterContractMismatch { expected, actual } = error else {
        panic!("expected counter mismatch, got {error}");
    };
    assert_eq!(
        expected,
        format!("router/1:sha256:{}", card_b.digest().as_str())
    );
    assert_eq!(actual, card_a_counter);
    assert!(port_b.calls().is_empty());
    assert_eq!(backend.fake.as_ref().unwrap().calls(), backend_calls);
    assert_eq!(generation_ids(&kernel), generation_ids_before);
}

/// The IDs of the collection's generations in creation order.
fn generation_ids(kernel: &Kernel) -> Vec<i64> {
    kernel
        .database
        .generations(&kernel.scopes, &kernel.collection)
        .unwrap()
        .into_iter()
        .map(|generation| generation.id)
        .collect()
}

/// A recovery guard that preserves the observed published predecessor.
fn rebuild_guard(generation: i64) -> RebuildGuard {
    RebuildGuard {
        expected_published: Some(generation),
        generation_watermark: generation,
    }
}
