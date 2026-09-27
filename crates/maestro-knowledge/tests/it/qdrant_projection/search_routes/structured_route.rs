//! Exact document inventories stay separate from their supporting chunks.

use super::super::support::projection;
use super::{backends::fake, kernel::Kernel, models, support::cleanup};
use maestro_kernel::{
    document::{Disposition, Outcome},
    evidence::{Inventory, InventoryCount, RouteStatus},
    generation::GenerationState,
    retrieval::InventoryRequest,
};
use maestro_knowledge::search::{Query, routes::structured::search_structured};
use std::time::Duration;
use tokio::time::Instant;

#[tokio::test]
async fn inventory_counts_each_eligible_member_once_independent_of_limit() {
    let backend = fake();
    let kernel = Kernel::with_guides(2);
    let revisions = kernel
        .database
        .revisions(&kernel.scopes, &kernel.collection)
        .unwrap();
    for revision in revisions {
        kernel
            .database
            .record_disposition(&Disposition {
                revision_id: revision.id,
                outcome: Outcome::Accepted,
                reasons: Vec::new(),
                rule_ids: Vec::new(),
                decided_by: "test".to_owned(),
            })
            .unwrap();
    }
    let card = models::embedder(3);
    let port = models::Embedder::default();
    let qdrant = backend.client();
    let report = projection(&kernel, &qdrant, &port, &card)
        .publish(&kernel.chunk_set)
        .await
        .unwrap();
    let generation = kernel
        .database
        .generation(&kernel.scopes, report.generation)
        .unwrap()
        .unwrap();
    assert_eq!(generation.state, GenerationState::Published);
    let request = InventoryRequest::DocumentsBySet { set: None };
    let query = Query {
        generation: &generation,
        scopes: &kernel.scopes,
        text: "how many documents",
        limit: 1,
        version: None,
        qdrant: &qdrant,
    };
    let outcome = search_structured(
        &query,
        kernel.database.clone(),
        &request,
        Instant::now() + Duration::from_secs(5),
    )
    .await;
    assert_eq!(outcome.route.status, RouteStatus::Ok);
    assert_eq!(
        outcome.inventory,
        Some(Inventory::DocumentsBySet {
            set_filter: None,
            total_documents: 2,
            sets: vec![InventoryCount {
                value: None,
                documents: 2,
            }],
        })
    );
    assert_eq!(outcome.route.hits.len(), 2);
    assert!(
        outcome
            .route
            .hits
            .windows(2)
            .all(|pair| pair[0].chunk_id < pair[1].chunk_id)
    );
    assert!(
        outcome
            .route
            .hits
            .iter()
            .all(|hit| (hit.score - 1.0).abs() < f64::EPSILON)
    );

    let wide_query = Query { limit: 50, ..query };
    let wide = search_structured(
        &wide_query,
        kernel.database.clone(),
        &request,
        Instant::now() + Duration::from_secs(5),
    )
    .await;
    assert_eq!(outcome.inventory, wide.inventory);
    cleanup(&backend, &[&generation]).await;
}
