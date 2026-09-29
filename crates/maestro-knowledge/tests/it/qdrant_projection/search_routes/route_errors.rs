//! Refusals from admission, profiles and dense embedding.

use super::{
    backends::fake,
    kernel::Kernel,
    models::{self, Fault},
    support::{TEXT, collection_scopes, generation},
};
use maestro_kernel::{
    generation::{GenerationState, NewGeneration},
    retrieval::Clock,
};
use maestro_knowledge::{
    lexical,
    search::{
        Query, RuntimeClock, pin,
        routes::{
            dense::{Embedder, search_dense},
            error::{RouteError, VectorError},
            lexical::search_bm25,
        },
    },
};
use std::{error::Error as _, sync::Arc};
use tonic::Code;

#[tokio::test]
async fn lexical_route_refuses_a_generation_with_another_sparse_profile() {
    let backend = fake();
    let kernel = Kernel::with_guides(1);
    let card = models::embedder(3);
    let generation = generation(
        &kernel,
        &card,
        1,
        GenerationState::Published,
        "another-sparse-profile/1",
    );
    let qdrant = backend.client();
    let clock: Arc<dyn Clock> = Arc::new(RuntimeClock::current());
    let query = Query {
        generation: &generation,
        scopes: &collection_scopes(&kernel),
        text: TEXT,
        limit: 10,
        identifier_limit: 10,
        version: None,
        projection: &qdrant,
        clock: &clock,
    };
    assert!(matches!(
        search_bm25(&query).await,
        Err(RouteError::ProfileMismatch { .. })
    ));
}

#[tokio::test]
async fn dense_route_refuses_a_card_with_dimensions_from_another_generation() {
    let backend = fake();
    let kernel = Kernel::with_guides(1);
    let recorded = models::embedder(3);
    let mismatched = models::embedder(4);
    let port = models::Embedder::default();
    let generation = generation(
        &kernel,
        &recorded,
        1,
        GenerationState::Published,
        lexical::PROFILE,
    );
    let qdrant = backend.client();
    let clock: Arc<dyn Clock> = Arc::new(RuntimeClock::current());
    let query = Query {
        generation: &generation,
        scopes: &collection_scopes(&kernel),
        text: TEXT,
        limit: 10,
        identifier_limit: 10,
        version: None,
        projection: &qdrant,
        clock: &clock,
    };
    let embedder = Embedder {
        port: &port,
        card: &mismatched,
    };
    assert!(matches!(
        search_dense(&query, &embedder).await,
        Err(RouteError::ProfileMismatch { .. })
    ));
    assert!(
        port.calls().is_empty(),
        "mismatched card must be refused before embedding"
    );
}

#[tokio::test]
async fn no_free_room_is_typed_and_does_not_touch_qdrant() {
    let backend = fake();
    backend
        .fake
        .as_ref()
        .unwrap()
        .refuse_next("query", Code::Unavailable);
    let kernel = Kernel::with_guides(1);
    let card = models::embedder(3);
    let port = models::Embedder::default();
    port.fail(0, Fault::Unavailable);
    let generation = generation(
        &kernel,
        &card,
        1,
        GenerationState::Published,
        lexical::PROFILE,
    );
    let qdrant = backend.client();
    let scopes = collection_scopes(&kernel);
    let clock: Arc<dyn Clock> = Arc::new(RuntimeClock::current());
    let query = Query {
        generation: &generation,
        scopes: &scopes,
        text: TEXT,
        limit: 10,
        identifier_limit: 10,
        version: None,
        projection: &qdrant,
        clock: &clock,
    };
    let failed = Embedder {
        port: &port,
        card: &card,
    };
    assert!(matches!(
        search_dense(&query, &failed).await,
        Err(RouteError::EmbedderUnavailable { .. })
    ));

    let succeeding = models::Embedder::default();
    let retry = Embedder {
        port: &succeeding,
        card: &card,
    };
    assert!(matches!(
        search_dense(&query, &retry).await,
        Err(RouteError::Projection(_))
    ));
}

#[tokio::test]
async fn dense_route_types_bad_dimensions_and_non_finite_vectors() {
    let backend = fake();
    let kernel = Kernel::with_guides(1);
    let card = models::embedder(3);
    let generation = generation(
        &kernel,
        &card,
        1,
        GenerationState::Published,
        lexical::PROFILE,
    );
    let qdrant = backend.client();
    let scopes = collection_scopes(&kernel);
    let clock: Arc<dyn Clock> = Arc::new(RuntimeClock::current());
    let query = Query {
        generation: &generation,
        scopes: &scopes,
        text: TEXT,
        limit: 10,
        identifier_limit: 10,
        version: None,
        projection: &qdrant,
        clock: &clock,
    };
    for fault in [Fault::Dimensions, Fault::NotANumber] {
        let port = models::Embedder::default();
        port.fail(0, fault);
        let embedder = Embedder {
            port: &port,
            card: &card,
        };
        let error = search_dense(&query, &embedder).await.unwrap_err();
        match (fault, error) {
            (
                Fault::Dimensions,
                RouteError::InvalidVector(VectorError::Dimensions {
                    expected: 3,
                    found: 2,
                }),
            )
            | (Fault::NotANumber, RouteError::InvalidVector(VectorError::NonFinite)) => {}
            (fault, error) => panic!("{fault:?} gave {error}"),
        }
    }
}

#[test]
fn pin_reports_unknown_and_unpublished_generations() {
    let kernel = Kernel::with_guides(1);
    let scopes = collection_scopes(&kernel);
    assert!(matches!(
        pin(&kernel.database, &scopes, &kernel.collection),
        Err(RouteError::UnknownGeneration { .. })
    ));
    let building = kernel
        .database
        .create_generation(&NewGeneration {
            collection_id: kernel.collection.clone(),
            chunk_set_id: kernel.chunk_set.clone(),
            embedding_profile: "dense/1:sha256:test".to_owned(),
            sparse_profile: lexical::PROFILE.to_owned(),
        })
        .unwrap();
    assert!(matches!(
        pin(&kernel.database, &scopes, &kernel.collection),
        Err(RouteError::UnpublishedGeneration {
            generation,
            state: GenerationState::Building,
        }) if generation == building.id
    ));
}

#[tokio::test]
async fn empty_scope_and_zero_limit_routes_return_without_model_or_qdrant() {
    let backend = fake();
    backend
        .fake
        .as_ref()
        .unwrap()
        .refuse_next("query", Code::Unavailable);
    let kernel = Kernel::with_guides(1);
    let card = models::embedder(3);
    let port = models::Embedder::default();
    let generation = generation(
        &kernel,
        &card,
        1,
        GenerationState::Published,
        lexical::PROFILE,
    );
    let qdrant = backend.client();
    let empty_scopes = kernel.database.visible("ungranted-reader").unwrap();
    assert!(empty_scopes.is_empty());
    let scopes = collection_scopes(&kernel);
    let clock: Arc<dyn Clock> = Arc::new(RuntimeClock::current());
    let empty_scope_query = Query {
        generation: &generation,
        scopes: &empty_scopes,
        text: TEXT,
        limit: 10,
        identifier_limit: 10,
        version: None,
        projection: &qdrant,
        clock: &clock,
    };
    let zero_limit_query = Query {
        generation: &generation,
        scopes: &scopes,
        text: TEXT,
        limit: 0,
        identifier_limit: 0,
        version: None,
        projection: &qdrant,
        clock: &clock,
    };
    let embedder = Embedder {
        port: &port,
        card: &card,
    };

    assert!(
        search_dense(&empty_scope_query, &embedder)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(port.calls().is_empty(), "empty scopes must skip embedding");
    assert!(search_bm25(&empty_scope_query).await.unwrap().is_empty());
    assert!(
        search_dense(&zero_limit_query, &embedder)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(port.calls().is_empty(), "zero limit must skip embedding");
    assert!(search_bm25(&zero_limit_query).await.unwrap().is_empty());
}

#[test]
fn diagnostics_render_vector_details_and_preserve_their_cause() {
    let vector = VectorError::Dimensions {
        expected: 3,
        found: 2,
    };
    assert_eq!(vector.to_string(), "expected 3 dimensions, found 2");
    let route_error = RouteError::InvalidVector(vector);
    assert_eq!(
        route_error.to_string(),
        "invalid embedding vector: expected 3 dimensions, found 2"
    );
    assert!(route_error.source().is_some());
}
