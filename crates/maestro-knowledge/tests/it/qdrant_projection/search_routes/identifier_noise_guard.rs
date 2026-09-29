//! The identifier noise guard: a too-common identifier gets no payload votes,
//! and fusion takes no hits from an unavailable route.

use super::super::stopped_clock::on_stopped_clock;
use super::{
    backends::fake,
    fused_search::accept_all_revisions,
    identifier_route::{PublishedCommand, publish_command, publish_kernel},
    kernel::Kernel,
    support::{cleanup, identifier_point, upsert},
};
use maestro_kernel::evidence::{RequestBudget, RouteStatus};
use maestro_knowledge::{
    query::understand,
    search::{
        DroppedIdentifier, EvidenceInput, IdentifierOutcome, Query, Route, RouteOutcome,
        SearchConfiguration, SearchContext, SearchRequest,
        evidence::EvidenceSettings,
        routes::{
            dense::Embedder,
            identifier::{search_identifiers, search_identifiers_guarded},
        },
        search,
    },
};
use qdrant_client::qdrant::{condition::ConditionOneOf, r#match::MatchValue};
use std::{future, time::Duration};
use tokio::time::Instant;
use tonic::Code;

/// Twenty-one guides whose every chunk carries the version `9.0.22`, one more than the
/// route's limit of twenty, and a lead chunk carrying the rare `ERR-042`.
pub(super) fn crowded_kernel() -> Kernel {
    Kernel::with_changed_guides(22, &|kernel, guide, mut chunks| {
        let text: &[u8] = if guide == 0 {
            b"The ctm command repairs the local cache at ERR-042."
        } else {
            b"Install version 9.0.22 of the tool."
        };
        for chunk in &mut chunks {
            chunk.digest = kernel.put(text);
        }
        chunks
    })
}

/// Searches one literal with separate general and identifier candidate caps.
async fn identifier_search_limits(
    fixture: &PublishedCommand,
    text: &str,
    route_limit: usize,
    identifier_limit: usize,
) -> RouteOutcome {
    let query = Query {
        generation: &fixture.generation,
        scopes: &fixture.kernel.scopes,
        text,
        limit: route_limit,
        identifier_limit,
        version: None,
        projection: &fixture.qdrant,
    };
    search_identifiers(
        &query,
        fixture.kernel.database.clone(),
        &understand(text),
        Instant::now() + Duration::from_secs(5),
    )
    .await
}

#[tokio::test]
async fn identifier_limit_bounds_identifier_route_candidates() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    let decoys = ["payload-only-a", "payload-only-b"]
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| {
            identifier_point(
                &format!("00000000-0000-4000-8000-{index:012x}"),
                chunk,
                "payload-revision",
                "workspace/default",
            )
        })
        .collect();
    upsert(&backend, &fixture.generation, decoys).await;
    let uncapped = identifier_search_limits(&fixture, "ERR-042", 100, 3).await;
    assert_eq!(uncapped.status, RouteStatus::Ok);
    assert_eq!(uncapped.hits.len(), 3);
    let capped = identifier_search_limits(&fixture, "ERR-042", 100, 2).await;
    assert_eq!(capped.status, RouteStatus::Ok);
    assert_eq!(capped.hits.len(), 2);
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn smaller_shared_route_limit_bounds_identifier_candidates_too() {
    let backend = fake();
    let kernel = crowded_kernel();
    accept_all_revisions(&kernel);
    let fixture = publish_kernel(&backend, kernel).await;
    let result = identifier_search_limits(&fixture, "9.0.22", 1, 20).await;
    assert_eq!(
        result.status,
        RouteStatus::Unavailable("kernel: identifier too common".to_owned())
    );
    assert!(result.hits.len() <= 1);
    cleanup(&backend, &[&fixture.generation]).await;
}

/// Runs the identifier route on `text` with the noise guard on.
async fn guarded(fixture: &PublishedCommand, text: &str) -> IdentifierOutcome {
    search_identifiers_guarded(
        &query(fixture, text),
        fixture.kernel.database.clone(),
        &understand(text),
        Instant::now() + Duration::from_secs(5),
    )
    .await
}

/// A route query over the fixture's generation, with no version filter.
fn query<'a>(fixture: &'a PublishedCommand, text: &'a str) -> Query<'a> {
    Query {
        generation: &fixture.generation,
        scopes: &fixture.kernel.scopes,
        text,
        limit: 20,
        identifier_limit: 20,
        version: None,
        projection: &fixture.qdrant,
    }
}

/// The identifiers the last payload scroll asked Qdrant for.
fn scrolled_identifiers(backend: &super::backends::Backend) -> Vec<String> {
    let filters = backend.fake.as_ref().unwrap().scroll_filters();
    let filter = filters.last().expect("a payload scroll ran");
    filter
        .must
        .iter()
        .find_map(|condition| {
            let Some(ConditionOneOf::Field(field)) = condition.condition_one_of.as_ref() else {
                return None;
            };
            match field.r#match.as_ref()?.match_value.as_ref()? {
                MatchValue::Keywords(values) if field.key == "identifiers" => {
                    Some(values.strings.clone())
                }
                _ => None,
            }
        })
        .expect("the scroll filters on identifiers")
}

fn too_common(identifier: &str) -> DroppedIdentifier {
    DroppedIdentifier {
        identifier: identifier.to_owned(),
        reason: "identifier too common".to_owned(),
    }
}

#[tokio::test]
async fn a_too_common_identifier_gets_no_payload_votes_with_the_guard_on() {
    let backend = fake();
    let fixture = publish_kernel(&backend, crowded_kernel()).await;
    let scrolls = || backend.fake.as_ref().unwrap().scroll_filters().len();

    let before = scrolls();
    let off = search_identifiers(
        &query(&fixture, "9.0.22"),
        fixture.kernel.database.clone(),
        &understand("9.0.22"),
        Instant::now() + Duration::from_secs(5),
    )
    .await;
    assert_eq!(
        off.status,
        RouteStatus::Unavailable("kernel: identifier too common".to_owned())
    );
    assert!(!off.hits.is_empty(), "off keeps the payload leg's votes");
    assert!(scrolls() > before);

    let before = scrolls();
    let on = guarded(&fixture, "9.0.22").await;
    assert_eq!(
        on.route.status,
        RouteStatus::Unavailable("kernel: identifier too common: \"9.0.22\"".to_owned())
    );
    assert!(on.route.hits.is_empty());
    assert_eq!(on.dropped, [too_common("9.0.22")]);
    assert_eq!(scrolls(), before, "the payload leg must not scroll");
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn a_rare_identifier_beside_a_too_common_one_still_votes() {
    let backend = fake();
    let fixture = publish_kernel(&backend, crowded_kernel()).await;

    let outcome = guarded(&fixture, "9.0.22 ERR-042").await;
    assert_eq!(outcome.route.status, RouteStatus::Ok);
    assert_eq!(outcome.dropped, [too_common("9.0.22")]);
    let chunks: Vec<_> = outcome
        .route
        .hits
        .iter()
        .map(|hit| hit.chunk_id.as_str())
        .collect();
    assert!(chunks.contains(&"chunk-0-lead"), "{chunks:?}");
    assert!(
        chunks.iter().all(|chunk| chunk.starts_with("chunk-0-")),
        "{chunks:?}"
    );
    assert_eq!(scrolled_identifiers(&backend), ["ERR-042"]);
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn a_lone_rare_identifier_votes_as_it_does_without_the_guard() {
    let backend = fake();
    let fixture = publish_command(&backend).await;

    let off = search_identifiers(
        &query(&fixture, "ERR-042"),
        fixture.kernel.database.clone(),
        &understand("ERR-042"),
        Instant::now() + Duration::from_secs(5),
    )
    .await;
    let on = guarded(&fixture, "ERR-042").await;
    assert_eq!(on.route, off);
    assert_eq!(on.route.status, RouteStatus::Ok);
    assert!(on.dropped.is_empty());
    assert!(
        on.route
            .hits
            .iter()
            .any(|hit| hit.chunk_id == "chunk-0-lead")
    );
    assert_eq!(scrolled_identifiers(&backend), ["ERR-042"]);
    cleanup(&backend, &[&fixture.generation]).await;
}

/// Searches `fixture` for `text` with lexical and identifier routes only,
/// with the noise guard as `guard` says, on a stopped clock.
async fn search_guarded(fixture: &PublishedCommand, text: &str, guard: bool) -> EvidenceInput {
    let context = SearchContext::<super::models::Embedder> {
        intent_expander: None,
        database: fixture.kernel.database.clone(),
        principal: "tester",
        projection: &fixture.qdrant,
        embedder: None::<Embedder<'_, super::models::Embedder>>,
        reranker: None,
        source_classes: None,
    };
    let request = SearchRequest {
        evidence: EvidenceSettings::default(),
        collection: &fixture.kernel.collection,
        text,
        version: None,
        budget: RequestBudget {
            deadline_ms: 5000,
            ..RequestBudget::default()
        },
        configuration: SearchConfiguration {
            dense_enabled: false,
            structured_enabled: false,
            rerank_enabled: false,
            identifier_noise_guard: guard,
            ..SearchConfiguration::default()
        },
    };
    on_stopped_clock(future::pending(), Box::pin(search(&context, &request)))
        .await
        .unwrap()
}

/// Whether any ranked candidate holds an identifier-route rank.
fn identifier_voted(input: &EvidenceInput) -> bool {
    input.ranked.iter().any(|ranked| {
        ranked
            .candidate
            .fused
            .ranks
            .contains_key(&Route::Identifier)
    })
}

#[tokio::test]
async fn the_guard_records_the_dropped_identifier_and_its_reason() {
    let backend = fake();
    let kernel = crowded_kernel();
    accept_all_revisions(&kernel);
    let fixture = publish_kernel(&backend, kernel).await;

    let off = search_guarded(&fixture, "install 9.0.22", false).await;
    assert!(identifier_voted(&off), "off fuses the payload noise");
    assert!(off.observations.identifiers_dropped.is_empty());

    let on = search_guarded(&fixture, "install 9.0.22", true).await;
    assert!(!identifier_voted(&on));
    assert_eq!(on.observations.identifiers_dropped, [too_common("9.0.22")]);
    assert_eq!(
        on.routes["identifier"],
        RouteStatus::Unavailable("kernel: identifier too common: \"9.0.22\"".to_owned())
    );
    assert!(on.known_gaps.iter().any(|gap| gap.contains("\"9.0.22\"")));
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn an_unavailable_routes_hits_are_never_fused_with_the_guard_on() {
    let backend = fake();
    let kernel = crowded_kernel();
    accept_all_revisions(&kernel);
    let fixture = publish_kernel(&backend, kernel).await;
    let fake = backend.fake.as_ref().unwrap();

    fake.refuse_next_n("scroll", Code::Internal, 10);
    let off = search_guarded(&fixture, "ERR-042", false).await;
    assert!(matches!(
        &off.routes["identifier"],
        RouteStatus::Unavailable(reason) if reason.starts_with("payload:")
    ));
    assert!(identifier_voted(&off), "off fuses the kernel leg's hits");

    fake.refuse_next_n("scroll", Code::Internal, 10);
    let on = search_guarded(&fixture, "ERR-042", true).await;
    assert!(matches!(
        &on.routes["identifier"],
        RouteStatus::Unavailable(reason) if reason.starts_with("payload:")
    ));
    assert!(
        on.observations.route_ranks[&Route::Identifier].contains(&"chunk-0-lead".to_owned()),
        "the kernel leg's hits are still observed"
    );
    assert!(!identifier_voted(&on), "an unavailable route must not vote");
    cleanup(&backend, &[&fixture.generation]).await;
}
