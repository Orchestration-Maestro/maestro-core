//! Behavior of dense and lexical searches over the generation-pinned collection.

use super::{
    backends::backends,
    kernel::Kernel,
    models,
    support::{
        TEXT, cleanup, client, collection, collection_scopes, create_collection, generation, point,
        upsert,
    },
};
use maestro_kernel::{
    gateway::{ModelPort, Room},
    generation::GenerationState,
    retrieval::Clock,
    scope::{Right, Scope},
};
use maestro_knowledge::{
    lexical::{self, query_vector},
    search::{
        Query, RuntimeClock,
        routes::{
            dense::{Embedder, search_dense},
            lexical::search_bm25,
        },
    },
};
use qdrant_client::qdrant::CreateAliasBuilder;
use std::{collections::BTreeSet, sync::Arc};

#[tokio::test]
async fn both_routes_apply_the_scope_filter_before_top_k() {
    for backend in backends("both_routes_apply_the_scope_filter_before_top_k") {
        let kernel = Kernel::with_guides(1);
        let card = models::embedder(3);
        let port = models::Embedder::default();
        let scopes = collection_scopes(&kernel);
        let qdrant = backend.client();
        let generation = generation(
            &kernel,
            &card,
            1,
            GenerationState::Published,
            lexical::PROFILE,
        );
        create_collection(&backend, &generation, &card).await;

        let dense_query = port
            .embed(&card, Room::Free, &[TEXT.to_owned()])
            .await
            .unwrap()
            .remove(0);
        let unrelated: Vec<f32> = dense_query.iter().map(|value| -*value).collect();
        let lexical_query = query_vector(TEXT);
        let denied_scope = "workspace/default/collection/other";
        upsert(
            &backend,
            &generation,
            vec![
                point(
                    1,
                    "denied",
                    "revision-denied",
                    denied_scope,
                    &dense_query,
                    lexical_query.indices(),
                    &vec![20.0; lexical_query.values().len()],
                ),
                point(
                    2,
                    "allowed",
                    "revision-allowed",
                    &kernel.collection_scope(),
                    &unrelated,
                    lexical_query.indices(),
                    &vec![0.01; lexical_query.values().len()],
                ),
            ],
        )
        .await;

        let clock: Arc<dyn Clock> = Arc::new(RuntimeClock::current());
        let query = Query {
            generation: &generation,
            scopes: &scopes,
            text: TEXT,
            limit: 1,
            identifier_limit: 1,
            version: None,
            projection: &qdrant,
            clock: &clock,
        };
        let embedder = Embedder {
            port: &port,
            card: &card,
        };
        let dense = search_dense(&query, &embedder).await.unwrap();
        let lexical = search_bm25(&query).await.unwrap();
        assert_eq!(dense.len(), 1, "{}", backend.name);
        assert_eq!(dense[0].chunk_id, "allowed", "{}", backend.name);
        assert_eq!(lexical.len(), 1, "{}", backend.name);
        assert_eq!(lexical[0].chunk_id, "allowed", "{}", backend.name);
        cleanup(&backend, &[&generation]).await;
    }
}

#[tokio::test]
async fn dense_route_embeds_the_card_formatted_query() {
    for backend in backends("dense_route_embeds_the_card_formatted_query") {
        let kernel = Kernel::with_guides(1);
        let card = models::v2_embedder_with_query_prefix();
        let port = models::Embedder::default();
        let scopes = collection_scopes(&kernel);
        let qdrant = backend.client();
        let generation = generation(
            &kernel,
            &card,
            1,
            GenerationState::Published,
            lexical::PROFILE,
        );
        create_collection(&backend, &generation, &card).await;
        let clock: Arc<dyn Clock> = Arc::new(RuntimeClock::current());
        let query = Query {
            generation: &generation,
            scopes: &scopes,
            text: TEXT,
            limit: 1,
            identifier_limit: 1,
            version: None,
            projection: &qdrant,
            clock: &clock,
        };
        let embedder = Embedder {
            port: &port,
            card: &card,
        };

        assert!(search_dense(&query, &embedder).await.unwrap().is_empty());
        assert_eq!(port.calls(), [vec![card.format_query(TEXT)]]);
        assert_ne!(port.calls()[0][0], TEXT);
        cleanup(&backend, &[&generation]).await;
    }
}

#[tokio::test]
async fn a_second_source_grant_cannot_search_another_sources_duplicate() {
    for backend in backends("a_second_source_grant_cannot_search_another_sources_duplicate") {
        let kernel = Kernel::with_guides(1);
        let card = models::embedder(3);
        let port = models::Embedder::default();
        let qdrant = backend.client();
        let report = super::super::support::projection(&kernel, &qdrant, &port, &card)
            .publish(&kernel.chunk_set)
            .await
            .unwrap();
        let generation = kernel
            .database
            .generation(&kernel.scopes, report.generation)
            .unwrap()
            .unwrap();
        let chunk = kernel.chunks().remove(0);
        let text = kernel.input(&chunk);
        let mirror: Scope = format!("{}/source/mirror", kernel.collection_scope())
            .parse()
            .unwrap();
        kernel
            .database
            .grant("mirror-reader", &mirror, Right::Read, "test")
            .unwrap();
        let scopes = kernel.database.visible("mirror-reader").unwrap();
        let clock: Arc<dyn Clock> = Arc::new(RuntimeClock::current());
        let query = Query {
            generation: &generation,
            scopes: &scopes,
            text: &text,
            limit: 10,
            identifier_limit: 10,
            version: None,
            projection: &qdrant,
            clock: &clock,
        };
        let embedder = Embedder {
            port: &port,
            card: &card,
        };
        let dense = search_dense(&query, &embedder).await.unwrap();
        let lexical = search_bm25(&query).await.unwrap();
        assert!(
            dense.is_empty() && lexical.is_empty(),
            "{}: dense {dense:?}, lexical {lexical:?}",
            backend.name
        );
        backend
            .cleanup(&kernel.collection, [report.generation])
            .await;
    }
}

#[tokio::test]
async fn routes_pinned_to_an_older_generation_ignore_the_moved_alias() {
    for backend in backends("routes_pinned_to_an_older_generation_ignore_the_moved_alias") {
        let kernel = Kernel::with_guides(1);
        let card = models::embedder(3);
        let port = models::Embedder::default();
        let scopes = collection_scopes(&kernel);
        let qdrant = backend.client();
        let old = generation(
            &kernel,
            &card,
            1,
            GenerationState::Retired,
            lexical::PROFILE,
        );
        let new = generation(
            &kernel,
            &card,
            2,
            GenerationState::Published,
            lexical::PROFILE,
        );
        create_collection(&backend, &old, &card).await;
        create_collection(&backend, &new, &card).await;
        let dense_query = port
            .embed(&card, Room::Free, &[TEXT.to_owned()])
            .await
            .unwrap()
            .remove(0);
        let sparse = query_vector(TEXT);
        for (generation, chunk) in [(&old, "older"), (&new, "newer")] {
            upsert(
                &backend,
                generation,
                vec![point(
                    usize::try_from(generation.id).unwrap(),
                    chunk,
                    "revision",
                    &kernel.collection_scope(),
                    &dense_query,
                    sparse.indices(),
                    sparse.values(),
                )],
            )
            .await;
        }
        let alias = format!("maestro-{}", kernel.collection);
        client(&backend)
            .create_alias(CreateAliasBuilder::new(collection(&new), alias))
            .await
            .unwrap();

        let clock: Arc<dyn Clock> = Arc::new(RuntimeClock::current());
        let query = Query {
            generation: &old,
            scopes: &scopes,
            text: TEXT,
            limit: 1,
            identifier_limit: 1,
            version: None,
            projection: &qdrant,
            clock: &clock,
        };
        let embedder = Embedder {
            port: &port,
            card: &card,
        };
        assert_eq!(
            search_dense(&query, &embedder).await.unwrap()[0].chunk_id,
            "older"
        );
        assert_eq!(search_bm25(&query).await.unwrap()[0].chunk_id, "older");
        cleanup(&backend, &[&old, &new]).await;
    }
}

#[tokio::test]
async fn each_route_caps_results_at_k_and_deduplicates_chunks() {
    for backend in backends("each_route_caps_results_at_k_and_deduplicates_chunks") {
        let kernel = Kernel::with_guides(1);
        let card = models::embedder(3);
        let port = models::Embedder::default();
        let scopes = collection_scopes(&kernel);
        let qdrant = backend.client();
        let generation = generation(
            &kernel,
            &card,
            1,
            GenerationState::Published,
            lexical::PROFILE,
        );
        create_collection(&backend, &generation, &card).await;
        let dense_query = port
            .embed(&card, Room::Free, &[TEXT.to_owned()])
            .await
            .unwrap()
            .remove(0);
        let mut near = dense_query.clone();
        near[0] += 1.0;
        let far: Vec<f32> = dense_query.iter().map(|value| -*value).collect();
        let sparse = query_vector(TEXT);
        let scaled = |factor: f32| -> Vec<f32> {
            sparse.values().iter().map(|value| value * factor).collect()
        };
        let scope = kernel.collection_scope();
        upsert(
            &backend,
            &generation,
            vec![
                point(
                    1,
                    "duplicate",
                    "first-revision",
                    &scope,
                    &dense_query,
                    sparse.indices(),
                    &scaled(10.0),
                ),
                point(
                    2,
                    "duplicate",
                    "second-revision",
                    &scope,
                    &dense_query,
                    sparse.indices(),
                    &scaled(9.0),
                ),
                point(
                    3,
                    "other",
                    "revision-other",
                    &scope,
                    &near,
                    sparse.indices(),
                    &scaled(2.0),
                ),
                point(
                    4,
                    "last",
                    "revision-last",
                    &scope,
                    &far,
                    sparse.indices(),
                    &scaled(0.1),
                ),
            ],
        )
        .await;

        let clock: Arc<dyn Clock> = Arc::new(RuntimeClock::current());
        let query = Query {
            generation: &generation,
            scopes: &scopes,
            text: TEXT,
            limit: 3,
            identifier_limit: 3,
            version: None,
            projection: &qdrant,
            clock: &clock,
        };
        let embedder = Embedder {
            port: &port,
            card: &card,
        };
        for results in [
            search_dense(&query, &embedder).await.unwrap(),
            search_bm25(&query).await.unwrap(),
        ] {
            assert!(
                results.len() <= query.limit,
                "{}: {results:?}",
                backend.name
            );
            let chunks: BTreeSet<_> = results.iter().map(|hit| &hit.chunk_id).collect();
            assert_eq!(chunks.len(), results.len(), "{}: {results:?}", backend.name);
            assert!(results.iter().any(|hit| hit.chunk_id == "duplicate"));
        }
        cleanup(&backend, &[&generation]).await;
    }
}
