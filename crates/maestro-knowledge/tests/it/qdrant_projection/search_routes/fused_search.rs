//! The public search handoff fuses available routes when dense metadata is absent.

use super::super::support::projection;
use super::{backends::fake, kernel::Kernel, models, support::cleanup};
use maestro_kernel::{
    document::{Disposition, Outcome},
    evidence::{RequestBudget, RouteStatus},
    gateway::Role,
};
use maestro_knowledge::{
    query::Family,
    search::{Reranker, SearchContext, SearchError, SearchRequest, search},
};
use std::collections::HashSet;
use tokio::time::Instant;

pub(super) fn searchable_scheduler_kernel() -> Kernel {
    let kernel = Kernel::with_changed_guides(1, &|kernel, guide, mut chunks| {
        if guide == 0 {
            chunks[0].digest =
                kernel.put(b"The scheduler runs job-0 in version 9.0.22 and reports ERR-042.");
            let filler = kernel.put(b"unrelated frequency padding");
            for index in 0..16 {
                let mut chunk = chunks[0].clone();
                chunk.id = format!("chunk-0-frequency-filler-{index}");
                chunk.section_id = None;
                chunk.digest = filler.clone();
                chunks.push(chunk);
            }
        }
        chunks
    });
    accept_all_revisions(&kernel);
    kernel
}

/// Marks each revision represented in the fixture as eligible for search.
pub(super) fn accept_all_revisions(kernel: &Kernel) {
    let mut revisions = HashSet::new();
    for chunk in kernel.chunks() {
        if revisions.insert(chunk.revision_id.clone()) {
            kernel
                .database
                .record_disposition(&Disposition {
                    revision_id: chunk.revision_id,
                    outcome: Outcome::Accepted,
                    reasons: Vec::new(),
                    rule_ids: Vec::new(),
                    decided_by: "test".to_owned(),
                })
                .unwrap();
        }
    }
}

#[tokio::test]
async fn undocumented_question_version_does_not_filter_search_results() {
    for backend in
        super::backends::backends("undocumented_question_version_does_not_filter_search_results")
    {
        let kernel = searchable_scheduler_kernel();
        let card = models::embedder(3);
        let port = models::Embedder::default();
        let qdrant = backend.client();
        let report = projection(&kernel, &qdrant, &port, &card)
            .publish(&kernel.chunk_set)
            .await
            .unwrap();
        let context: SearchContext<'_, models::Embedder> = SearchContext {
            intent_expander: None,
            database: kernel.database.clone(),
            principal: "tester",
            qdrant: &qdrant,
            embedder: None,
            reranker: None,
            source_classes: None,
        };
        let request = SearchRequest::new(
            &kernel.collection,
            "scheduler 9.0.22.100",
            None,
            RequestBudget {
                deadline_ms: 5000,
                ..RequestBudget::default()
            },
        );
        let result = Box::pin(search(&context, &request)).await.unwrap();
        assert_eq!(result.understood.version.as_deref(), Some("9.0.22.100"));
        assert_eq!(result.version, None);
        assert!(!result.routes.contains_key("structured"));
        assert!(!result.ranked.is_empty(), "{}", backend.name);
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
}

#[tokio::test]
async fn explicit_version_overrides_question_version() {
    for backend in super::backends::backends("explicit_version_overrides_question_version") {
        let kernel = searchable_scheduler_kernel();
        let card = models::embedder(3);
        let port = models::Embedder::default();
        let qdrant = backend.client();
        let report = projection(&kernel, &qdrant, &port, &card)
            .publish(&kernel.chunk_set)
            .await
            .unwrap();
        let context: SearchContext<'_, models::Embedder> = SearchContext {
            intent_expander: None,
            database: kernel.database.clone(),
            principal: "tester",
            qdrant: &qdrant,
            embedder: None,
            reranker: None,
            source_classes: None,
        };
        let request = SearchRequest::new(
            &kernel.collection,
            "scheduler 9.0.22.100",
            Some("9.0.22"),
            RequestBudget {
                deadline_ms: 5000,
                ..RequestBudget::default()
            },
        );
        let result = Box::pin(search(&context, &request)).await.unwrap();
        assert_eq!(result.understood.version.as_deref(), Some("9.0.22.100"));
        assert_eq!(result.version.as_deref(), Some("9.0.22"));
        assert!(!result.ranked.is_empty(), "{}", backend.name);
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
}

#[tokio::test]
async fn missing_explicit_version_is_reported_as_a_known_gap() {
    let backend = fake();
    let kernel = searchable_scheduler_kernel();
    let card = models::embedder(3);
    let port = models::Embedder::default();
    let qdrant = backend.client();
    let report = projection(&kernel, &qdrant, &port, &card)
        .publish(&kernel.chunk_set)
        .await
        .unwrap();
    let context: SearchContext<'_, models::Embedder> = SearchContext {
        intent_expander: None,
        database: kernel.database.clone(),
        principal: "tester",
        qdrant: &qdrant,
        embedder: None,
        reranker: None,
        source_classes: None,
    };
    let request = SearchRequest::new(
        &kernel.collection,
        "scheduler 9.0.22",
        Some("99.99"),
        RequestBudget {
            deadline_ms: 5000,
            ..RequestBudget::default()
        },
    );
    let result = Box::pin(search(&context, &request)).await.unwrap();
    assert!(result.ranked.is_empty());
    assert_eq!(result.version.as_deref(), Some("99.99"));
    assert!(result.known_gaps.iter().any(|gap| gap.contains("99.99")));
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
async fn malformed_inventory_filter_degrades_only_the_structured_route() {
    let backend = fake();
    let kernel = searchable_scheduler_kernel();
    let card = models::embedder(3);
    let port = models::Embedder::default();
    let qdrant = backend.client();
    let report = projection(&kernel, &qdrant, &port, &card)
        .publish(&kernel.chunk_set)
        .await
        .unwrap();
    let context: SearchContext<'_, models::Embedder> = SearchContext {
        intent_expander: None,
        database: kernel.database.clone(),
        principal: "tester",
        qdrant: &qdrant,
        embedder: None,
        reranker: None,
        source_classes: None,
    };
    let request = SearchRequest::new(
        &kernel.collection,
        r#"how many documents in set "ERR-042" trailing"#,
        None,
        RequestBudget {
            deadline_ms: 5000,
            ..RequestBudget::default()
        },
    );

    let result = Box::pin(search(&context, &request)).await.unwrap();
    assert_eq!(
        result.routes.get("structured"),
        Some(&RouteStatus::Unavailable(
            "inventory set must be one nonempty JSON string".to_owned()
        ))
    );
    assert_eq!(result.routes.get("identifier"), Some(&RouteStatus::Ok));
    assert!(
        result
            .ranked
            .iter()
            .any(|item| { item.candidate.fused.chunk_id == "chunk-0-lead" })
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
}

#[tokio::test]
async fn mismatched_route_revision_is_refused_before_rerank() {
    let backend = fake();
    let kernel = searchable_scheduler_kernel();
    let publish_port = models::Embedder::default();
    let embedder_card = models::embedder(3);
    let qdrant = backend.client();
    let report = projection(&kernel, &qdrant, &publish_port, &embedder_card)
        .publish(&kernel.chunk_set)
        .await
        .unwrap();
    let collection = super::super::support::collection_of(&kernel, report.generation);
    let fake = backend.fake.as_ref().unwrap();
    fake.set_payload_text(
        &collection,
        super::super::support::point_id("chunk-0-lead"),
        "revision_id",
        "foreign-revision",
    );
    let rerank_port = models::Embedder::default();
    let reranker_card = models::card(Role::Reranker, 0);
    let context: SearchContext<'_, models::Embedder> = SearchContext {
        intent_expander: None,
        database: kernel.database.clone(),
        principal: "tester",
        qdrant: &qdrant,
        embedder: None,
        reranker: Some(Reranker {
            port: &rerank_port,
            card: &reranker_card,
        }),
        source_classes: None,
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
    assert!(matches!(
        Box::pin(search(&context, &request)).await,
        Err(SearchError::EvidenceLoad { .. })
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
async fn missing_embedder_degrades_dense_but_fuses_other_routes() {
    let backend = fake();
    let kernel = Kernel::with_changed_guides(1, &|kernel, guide, mut chunks| {
        if guide != 0 {
            return chunks;
        }
        chunks[0].digest = kernel.put(b"The ctm command repairs the local cache.");
        let filler = kernel.put(b"unrelated frequency padding");
        for index in 0..16 {
            let mut chunk = chunks[0].clone();
            chunk.id = format!("chunk-0-frequency-filler-{index}");
            chunk.section_id = None;
            chunk.digest = filler.clone();
            chunks.push(chunk);
        }
        chunks
    });
    accept_all_revisions(&kernel);
    let card = models::embedder(3);
    let port = models::Embedder::default();
    let qdrant = backend.client();
    let report = projection(&kernel, &qdrant, &port, &card)
        .publish(&kernel.chunk_set)
        .await
        .unwrap();
    let calls_before_search = port.calls().len();
    let context: SearchContext<'_, models::Embedder> = SearchContext {
        intent_expander: None,
        database: kernel.database.clone(),
        principal: "tester",
        qdrant: &qdrant,
        embedder: None,
        reranker: None,
        source_classes: None,
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
    let result = Box::pin(search(&context, &request)).await.unwrap();
    assert_eq!(result.understood.identifiers[0].family, Family::Command);
    assert!(matches!(
        result.routes.get("dense"),
        Some(RouteStatus::Unavailable(reason))
            if reason == "no embedder card for the published generation's profile"
    ));
    assert_eq!(result.routes.get("identifier"), Some(&RouteStatus::Ok));
    assert_eq!(result.routes.get("lexical"), Some(&RouteStatus::Ok));
    assert!(matches!(
        result.routes.get("rerank"),
        Some(RouteStatus::Unavailable(reason)) if reason == "no reranker configured"
    ));
    assert_eq!(
        port.calls().len(),
        calls_before_search,
        "dense was disabled"
    );
    assert!(!result.ranked.is_empty());
    assert!(
        result
            .ranked
            .iter()
            .any(|ranked| { ranked.candidate.text == "The ctm command repairs the local cache." })
    );
    assert_eq!(result.budget.deadline_ms, 5000);
    assert!(result.deadline > Instant::now());
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
