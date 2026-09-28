//! Request configuration controls the real fused search pipeline.

use super::super::support::projection;
use super::{
    backends::fake, fused_search::searchable_scheduler_kernel, kernel::Kernel, models,
    support::cleanup,
};
use maestro_kernel::evidence::RequestBudget;
use maestro_kernel::{
    evidence::RouteStatus,
    gateway::{ModelCard, Role},
};
use maestro_knowledge::search::evidence::EvidenceSettings;
use maestro_knowledge::{
    index::Qdrant,
    search::{
        DISABLED_BY_CONFIGURATION, EvidenceInput, Reranker, Route, SearchConfiguration,
        SearchContext, SearchRequest, routes::dense::Embedder, search,
    },
};

pub(super) struct Published {
    pub(super) backend: super::backends::Backend,
    pub(super) kernel: Kernel,
    pub(super) qdrant: Qdrant,
    pub(super) embedder_card: ModelCard,
    pub(super) port: models::Embedder,
    generation: i64,
}

pub(super) async fn published() -> Published {
    let backend = fake();
    let kernel = searchable_scheduler_kernel();
    let embedder_card = models::embedder(3);
    let port = models::Embedder::default();
    let qdrant = backend.client();
    let report = projection(&kernel, &qdrant, &port, &embedder_card)
        .publish(&kernel.chunk_set)
        .await
        .unwrap();
    Published {
        backend,
        kernel,
        qdrant,
        embedder_card,
        port,
        generation: report.generation,
    }
}

pub(super) fn context<'a>(
    fixture: &'a Published,
    reranker_card: Option<&'a ModelCard>,
) -> SearchContext<'a, models::Embedder> {
    SearchContext {
        intent_expander: None,
        database: fixture.kernel.database.clone(),
        principal: "tester",
        qdrant: &fixture.qdrant,
        embedder: Some(Embedder {
            port: &fixture.port,
            card: &fixture.embedder_card,
        }),
        reranker: reranker_card.map(|card| Reranker {
            port: &fixture.port,
            card,
        }),
    }
}

fn request<'a>(
    fixture: &'a Published,
    text: &'a str,
    configuration: SearchConfiguration,
) -> SearchRequest<'a> {
    SearchRequest {
        evidence: EvidenceSettings::default(),
        collection: &fixture.kernel.collection,
        text,
        version: None,
        budget: RequestBudget {
            deadline_ms: 5000,
            ..RequestBudget::default()
        },
        configuration,
    }
}

fn off() -> SearchConfiguration {
    SearchConfiguration {
        dense_enabled: false,
        lexical_enabled: false,
        identifier_enabled: false,
        structured_enabled: false,
        rerank_enabled: false,
        ..SearchConfiguration::default()
    }
}

async fn run(fixture: &Published, text: &str, configuration: SearchConfiguration) -> EvidenceInput {
    let context = context(fixture, None);
    Box::pin(search(&context, &request(fixture, text, configuration)))
        .await
        .unwrap()
}

pub(super) async fn clean(fixture: &Published) {
    let generation = fixture
        .kernel
        .database
        .generation(&fixture.kernel.scopes, fixture.generation)
        .unwrap()
        .unwrap();
    cleanup(&fixture.backend, &[&generation]).await;
}

#[tokio::test]
async fn each_route_switch_changes_calls_or_search_results() {
    let fixture = published().await;

    let dense_calls = fixture.port.calls().len();
    let dense_config = SearchConfiguration {
        dense_enabled: true,
        ..off()
    };
    let dense = run(&fixture, "scheduler", dense_config).await;
    assert!(fixture.port.calls().len() > dense_calls);
    assert!(!dense.observations.route_ranks[&Route::Dense].is_empty());
    assert!(
        !dense
            .observations
            .route_ranks
            .contains_key(&Route::Structured)
    );
    let dense_calls = fixture.port.calls().len();
    let dense_off = run(&fixture, "scheduler", off()).await;
    assert_eq!(fixture.port.calls().len(), dense_calls);
    assert_eq!(
        dense_off.routes["dense"],
        RouteStatus::Unavailable(DISABLED_BY_CONFIGURATION.to_owned())
    );
    let dense_and_rerank_off = SearchConfiguration {
        lexical_enabled: true,
        identifier_enabled: true,
        ..off()
    };
    let model_calls = fixture.port.calls().len();
    let rerank_calls = fixture.port.rerank_calls();
    let lexical_and_identifier = run(&fixture, "scheduler ERR-042", dense_and_rerank_off).await;
    assert!(!lexical_and_identifier.ranked.is_empty());
    assert_eq!(fixture.port.calls().len(), model_calls);
    assert_eq!(fixture.port.rerank_calls(), rerank_calls);

    let lexical_config = SearchConfiguration {
        lexical_enabled: true,
        ..off()
    };
    let lexical = run(&fixture, "scheduler", lexical_config).await;
    assert!(!lexical.ranked.is_empty());
    assert!(!lexical.observations.route_ranks[&Route::Lexical].is_empty());
    let before_lexical_off = fixture.backend.fake.as_ref().unwrap().calls().len();
    let lexical_off = run(&fixture, "scheduler", off()).await;
    assert_eq!(
        fixture.backend.fake.as_ref().unwrap().calls().len(),
        before_lexical_off
    );
    assert_eq!(
        lexical_off.routes["lexical"],
        RouteStatus::Unavailable(DISABLED_BY_CONFIGURATION.to_owned())
    );

    let identifier_config = SearchConfiguration {
        identifier_enabled: true,
        ..off()
    };
    let identifier = run(&fixture, "ERR-042", identifier_config).await;
    assert!(!identifier.ranked.is_empty());
    assert!(!identifier.observations.route_ranks[&Route::Identifier].is_empty());
    let before_identifier_off = fixture.backend.fake.as_ref().unwrap().calls().len();
    let identifier_off = run(&fixture, "ERR-042", off()).await;
    assert_eq!(
        fixture.backend.fake.as_ref().unwrap().calls().len(),
        before_identifier_off
    );
    assert_eq!(
        identifier_off.routes["identifier"],
        RouteStatus::Unavailable(DISABLED_BY_CONFIGURATION.to_owned())
    );

    let structured_config = SearchConfiguration {
        structured_enabled: true,
        ..off()
    };
    let structured = run(&fixture, "how many documents", structured_config).await;
    assert!(structured.inventory.is_some());
    assert!(
        structured
            .observations
            .route_ranks
            .contains_key(&Route::Structured)
    );
    let structured_off = run(&fixture, "how many documents", off()).await;
    assert_eq!(structured_off.inventory, None);
    assert!(
        structured_off
            .observations
            .route_ranks
            .contains_key(&Route::Structured)
    );
    assert!(structured_off.observations.route_ranks[&Route::Structured].is_empty());
    assert_eq!(
        structured_off.routes["structured"],
        RouteStatus::Unavailable(DISABLED_BY_CONFIGURATION.to_owned())
    );
    assert!(
        structured_off
            .known_gaps
            .iter()
            .any(|gap| gap.contains("structured route unavailable"))
    );

    clean(&fixture).await;
}

#[tokio::test]
async fn weighted_fusion_and_stage_observations_follow_the_configured_pipeline() {
    let fixture = published().await;
    let hybrid = SearchConfiguration {
        dense_enabled: true,
        lexical_enabled: true,
        identifier_enabled: false,
        structured_enabled: false,
        rerank_enabled: false,
        ..SearchConfiguration::default()
    };
    let baseline = run(&fixture, "scheduler job", hybrid).await;
    let weight_changed = run(
        &fixture,
        "scheduler job",
        SearchConfiguration {
            dense_weight: 100.0,
            ..hybrid
        },
    )
    .await;
    let baseline_ids = baseline
        .ranked
        .iter()
        .map(|ranked| ranked.candidate.fused.chunk_id.as_str())
        .collect::<Vec<_>>();
    let weighted_ids = weight_changed
        .ranked
        .iter()
        .map(|ranked| ranked.candidate.fused.chunk_id.as_str())
        .collect::<Vec<_>>();
    assert_ne!(baseline_ids, weighted_ids);
    assert_eq!(
        baseline.observations.reranked_chunk_ids,
        baseline
            .ranked
            .iter()
            .map(|ranked| ranked.candidate.fused.chunk_id.clone())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        baseline.observations.route_ranks[&Route::Dense],
        run(
            &fixture,
            "scheduler job",
            SearchConfiguration {
                dense_enabled: true,
                ..off()
            }
        )
        .await
        .observations
        .route_ranks[&Route::Dense]
    );
    assert_eq!(
        baseline.observations.route_ranks[&Route::Lexical],
        run(
            &fixture,
            "scheduler job",
            SearchConfiguration {
                lexical_enabled: true,
                ..off()
            }
        )
        .await
        .observations
        .route_ranks[&Route::Lexical]
    );

    clean(&fixture).await;
}

#[tokio::test]
async fn reranked_and_failed_stage_ranks_are_retained_from_search() {
    let fixture = published().await;
    let hybrid = SearchConfiguration {
        dense_enabled: true,
        lexical_enabled: true,
        identifier_enabled: false,
        structured_enabled: false,
        rerank_enabled: false,
        ..SearchConfiguration::default()
    };
    let baseline = run(&fixture, "scheduler job", hybrid).await;
    let reranker_card = models::card(Role::Reranker, 3);
    let reranked = Box::pin(search(
        &context(&fixture, Some(&reranker_card)),
        &request(
            &fixture,
            "scheduler job",
            SearchConfiguration {
                rerank_enabled: true,
                ..hybrid
            },
        ),
    ))
    .await
    .unwrap();
    let reranked_ids = reranked
        .ranked
        .iter()
        .map(|ranked| ranked.candidate.fused.chunk_id.clone())
        .collect::<Vec<_>>();
    assert_ne!(baseline.observations.reranked_chunk_ids, reranked_ids);
    assert_eq!(reranked.observations.reranked_chunk_ids, reranked_ids);
    assert!(fixture.port.rerank_calls() > 0);

    let failing_card = models::card(Role::Embedder, 3);
    let failed = Box::pin(search(
        &context(&fixture, Some(&failing_card)),
        &request(
            &fixture,
            "scheduler job",
            SearchConfiguration {
                rerank_enabled: true,
                ..hybrid
            },
        ),
    ))
    .await
    .unwrap();
    assert_eq!(
        failed.routes["rerank"],
        RouteStatus::Unavailable("invalid_model_card".to_owned())
    );
    assert_eq!(
        failed.observations.reranked_chunk_ids,
        baseline.observations.reranked_chunk_ids
    );
    assert!(failed.ranked.iter().all(|ranked| ranked.score.is_none()));
    assert_eq!(fixture.port.rerank_calls(), 1);

    clean(&fixture).await;
}
