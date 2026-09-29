//! Search admission and generation-pinning acceptance tests.

use super::super::support::projection;
use super::{
    backends::fake,
    fused_search::searchable_scheduler_kernel,
    kernel::Kernel,
    models,
    support::{cleanup, generation, point, upsert},
};
use maestro_kernel::{
    document::{Disposition, Outcome},
    evidence::{Inventory, RequestBudget, RouteStatus},
    gateway::Role,
    generation::{GenerationState, NewGeneration},
    scope::{Right, Scope},
};
use maestro_knowledge::search::{
    Reranker, Route, SearchContext, SearchError, SearchRequest, search,
};
use qdrant_client::qdrant::{Distance, Modifier};

#[tokio::test]
async fn unknown_and_unpublished_collections_stop_before_model_or_qdrant_calls() {
    let backend = fake();
    let kernel = Kernel::with_guides(1);
    let card = models::embedder(3);
    let port = models::Embedder::default();
    let qdrant = backend.client();
    let context: SearchContext<'_, models::Embedder> = SearchContext {
        intent_expander: None,
        database: kernel.database.clone(),
        principal: "tester",
        qdrant: &qdrant,
        embedder: Some(maestro_knowledge::search::routes::dense::Embedder {
            port: &port,
            card: &card,
        }),
        reranker: None,
        source_classes: None,
        question_splitter: None,
        part_bridge: None,
    };
    for collection in ["missing-collection", kernel.collection.as_str()] {
        let request = SearchRequest::new(
            collection,
            "scheduler",
            None,
            RequestBudget {
                deadline_ms: 5000,
                ..RequestBudget::default()
            },
        );
        assert!(matches!(
            Box::pin(search(&context, &request)).await,
            Err(SearchError::Admission(_))
        ));
    }
    assert!(port.calls().is_empty());
    assert!(backend.fake.as_ref().unwrap().calls().is_empty());
}

#[tokio::test]
#[expect(
    clippy::too_many_lines,
    reason = "One gated scenario moves both the alias and the pinned kernel generation."
)]
async fn alias_and_generation_moves_after_admission_keep_search_pinned() {
    let backend = fake();
    let kernel = searchable_scheduler_kernel();
    let publish_port = models::Embedder::default();
    let embedder_card = models::embedder(3);
    let qdrant = backend.client();
    let report = projection(&kernel, &qdrant, &publish_port, &embedder_card)
        .publish(&kernel.chunk_set)
        .await
        .unwrap();
    let decoy_collection = format!("maestro-{}-g99", kernel.collection);
    backend
        .create(&decoy_collection, 3, Distance::Cosine, Some(Modifier::Idf))
        .await;
    let decoy_generation = generation(
        &kernel,
        &embedder_card,
        99,
        GenerationState::Building,
        "bm25-en-fr/1",
    );
    upsert(
        &backend,
        &decoy_generation,
        vec![point(
            99,
            "decoy-chunk",
            "decoy-revision",
            "workspace/default",
            &[1.0, 0.0, 0.0],
            &[],
            &[],
        )],
    )
    .await;

    let newer_set = format!("{}-newer", kernel.chunk_set);
    kernel.complete_another(&newer_set, 2, &|kernel, guide, mut chunks| {
        if guide == 0 {
            chunks[0].digest = kernel.put(b"new-generation-only");
        }
        chunks
    });
    let added_revision = kernel
        .database
        .revisions(&kernel.scopes, &kernel.collection)
        .unwrap()
        .into_iter()
        .find(|revision| revision.document_id.ends_with("-1"))
        .unwrap();
    kernel
        .database
        .record_disposition(&Disposition {
            revision_id: added_revision.id,
            outcome: Outcome::Accepted,
            reasons: Vec::new(),
            rule_ids: Vec::new(),
            decided_by: "test".to_owned(),
        })
        .unwrap();
    let newer_generation = kernel
        .database
        .create_generation(&NewGeneration {
            collection_id: kernel.collection.clone(),
            chunk_set_id: newer_set,
            embedding_profile: "embed:newer-generation".to_owned(),
            sparse_profile: "bm25-en-fr/1".to_owned(),
        })
        .unwrap();
    assert!(newer_generation.id > report.generation);

    let (search_port, gate) = models::Embedder::gated();
    let context: SearchContext<'_, models::Embedder> = SearchContext {
        intent_expander: None,
        database: kernel.database.clone(),
        principal: "tester",
        qdrant: &qdrant,
        embedder: Some(maestro_knowledge::search::routes::dense::Embedder {
            port: &search_port,
            card: &embedder_card,
        }),
        reranker: None,
        source_classes: None,
        question_splitter: None,
        part_bridge: None,
    };
    let request = SearchRequest::new(
        &kernel.collection,
        "How many documents?",
        None,
        RequestBudget {
            deadline_ms: 5000,
            ..RequestBudget::default()
        },
    );
    let search_future = search(&context, &request);
    tokio::pin!(search_future);
    tokio::select! {
        result = &mut search_future => panic!(
            "search completed before its embedder was released: {result:?}"
        ),
        () = gate.wait_started() => {}
    }

    backend
        .fake
        .as_ref()
        .unwrap()
        .alias_to(&super::super::support::alias_of(&kernel), &decoy_collection);
    let newer_chunks = kernel
        .database
        .chunks(&kernel.scopes, &newer_generation.chunk_set_id)
        .unwrap();
    kernel
        .database
        .verify_generation(
            newer_generation.id,
            u64::try_from(newer_chunks.len()).unwrap(),
        )
        .unwrap();
    kernel
        .database
        .publish_generation(newer_generation.id)
        .unwrap();
    assert_eq!(
        kernel
            .database
            .published_generation(&kernel.scopes, &kernel.collection)
            .unwrap()
            .unwrap()
            .id,
        newer_generation.id
    );
    gate.release();

    let result = search_future.await.unwrap();
    assert_eq!(result.generation.id, report.generation);
    assert_eq!(result.generation.chunk_set_id, kernel.chunk_set);
    assert_eq!(result.routes.get("dense"), Some(&RouteStatus::Ok));
    assert_eq!(result.routes.get("structured"), Some(&RouteStatus::Ok));
    assert!(matches!(
        result.inventory,
        Some(Inventory::DocumentsBySet {
            total_documents: 1,
            ..
        })
    ));
    assert!(result.ranked.iter().any(|item| {
        item.candidate.fused.ranks.contains_key(&Route::Dense)
            && item.candidate.text.contains("ERR-042")
    }));
    assert!(result.ranked.iter().all(|item| {
        item.candidate.fused.chunk_id != "decoy-chunk"
            && !item.candidate.text.contains("new-generation-only")
    }));
    assert_eq!(
        backend
            .fake
            .as_ref()
            .unwrap()
            .alias_target(&super::super::support::alias_of(&kernel)),
        Some(decoy_collection.clone())
    );
    cleanup(
        &backend,
        &[&kernel
            .database
            .generation(&kernel.scopes, report.generation)
            .unwrap()
            .unwrap()],
    )
    .await;
    backend.delete_collection(&decoy_collection).await;
}

#[tokio::test]
async fn a_permission_revocation_during_a_slow_route_aborts_the_handoff() {
    let backend = fake();
    let kernel = searchable_scheduler_kernel();
    let publish_port = models::Embedder::default();
    let embedder_card = models::embedder(3);
    let qdrant = backend.client();
    let report = projection(&kernel, &qdrant, &publish_port, &embedder_card)
        .publish(&kernel.chunk_set)
        .await
        .unwrap();
    let (search_port, gate) = models::Embedder::gated();
    let rerank_port = models::Embedder::default();
    let reranker_card = models::card(Role::Reranker, 0);
    let context: SearchContext<'_, models::Embedder> = SearchContext {
        intent_expander: None,
        database: kernel.database.clone(),
        principal: "tester",
        qdrant: &qdrant,
        embedder: Some(maestro_knowledge::search::routes::dense::Embedder {
            port: &search_port,
            card: &embedder_card,
        }),
        reranker: Some(Reranker {
            port: &rerank_port,
            card: &reranker_card,
        }),
        source_classes: None,
        question_splitter: None,
        part_bridge: None,
    };
    let request = SearchRequest::new(
        &kernel.collection,
        "`ctm`",
        None,
        RequestBudget {
            deadline_ms: 5000,
            ..RequestBudget::default()
        },
    );
    let search_future = search(&context, &request);
    tokio::pin!(search_future);
    tokio::select! {
        result = &mut search_future => panic!(
            "search completed before its embedder was released: {result:?}"
        ),
        () = gate.wait_started() => {}
    }
    let workspace: Scope = "workspace/default".parse().unwrap();
    kernel
        .database
        .revoke("tester", &workspace, Right::Read, "test")
        .unwrap();
    gate.release();

    assert!(matches!(
        search_future.await,
        Err(SearchError::PermissionsChanged)
    ));
    assert_eq!(rerank_port.rerank_calls(), 0);
    cleanup(
        &backend,
        &[&kernel
            .database
            .generation(&kernel.scopes, report.generation)
            .unwrap()
            .unwrap()],
    )
    .await;
}

#[tokio::test]
async fn a_new_grant_during_a_slow_route_aborts_the_handoff() {
    let backend = fake();
    let kernel = searchable_scheduler_kernel();
    let publish_port = models::Embedder::default();
    let embedder_card = models::embedder(3);
    let qdrant = backend.client();
    let report = projection(&kernel, &qdrant, &publish_port, &embedder_card)
        .publish(&kernel.chunk_set)
        .await
        .unwrap();
    let (search_port, gate) = models::Embedder::gated();
    let context: SearchContext<'_, models::Embedder> = SearchContext {
        intent_expander: None,
        database: kernel.database.clone(),
        principal: "tester",
        qdrant: &qdrant,
        embedder: Some(maestro_knowledge::search::routes::dense::Embedder {
            port: &search_port,
            card: &embedder_card,
        }),
        reranker: None,
        source_classes: None,
        question_splitter: None,
        part_bridge: None,
    };
    let request = SearchRequest::new(
        &kernel.collection,
        "`ctm`",
        None,
        RequestBudget {
            deadline_ms: 5000,
            ..RequestBudget::default()
        },
    );
    let search_future = search(&context, &request);
    tokio::pin!(search_future);
    tokio::select! {
        result = &mut search_future => panic!(
            "search completed before its embedder was released: {result:?}"
        ),
        () = gate.wait_started() => {}
    }
    let other_workspace: Scope = "workspace/other".parse().unwrap();
    kernel
        .database
        .grant("tester", &other_workspace, Right::Read, "test")
        .unwrap();
    gate.release();

    assert!(matches!(
        search_future.await,
        Err(SearchError::PermissionsChanged)
    ));
    cleanup(
        &backend,
        &[&kernel
            .database
            .generation(&kernel.scopes, report.generation)
            .unwrap()
            .unwrap()],
    )
    .await;
}
