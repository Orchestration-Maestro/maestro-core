//! Exact identifiers combine payload equality with the scoped kernel index.

use super::super::support::projection;
use super::{
    backends::{Backend, backends, fake},
    kernel::Kernel,
    models,
    support::{cleanup, identifier_point, upsert},
};
use maestro_kernel::{
    document::{Disposition, Outcome},
    evidence::RouteStatus,
    generation::{Generation, GenerationState},
    retrieval::IDENTIFIER_PROFILE,
};
use maestro_knowledge::{
    index::Qdrant,
    query::understand,
    search::{
        Query, RouteOutcome,
        routes::{identifier::search_identifiers, lexical::search_bm25},
    },
};
use qdrant_client::qdrant::{Condition, Filter, condition::ConditionOneOf, r#match::MatchValue};
use std::time::Duration;
use tokio::time::Instant;
use tonic::Code;

/// Published command input and the physical generation used by identifier tests.
pub(super) struct PublishedCommand {
    /// Kernel data and its trusted read scope.
    pub(super) kernel: Kernel,
    /// The Qdrant client of the test's backend.
    pub(super) qdrant: Qdrant,
    /// The ready generation pinned by every test query.
    pub(super) generation: Generation,
}

/// Publishes one accepted prepared input containing a bare command and error code.
pub(super) async fn publish_command(backend: &Backend) -> PublishedCommand {
    let kernel = Kernel::with_changed_guides(4, &|kernel, guide, mut chunks| {
        if guide == 0 {
            chunks[0].digest = kernel.put(b"The ctm command repairs the local cache at ERR-042.");
        }
        chunks
    });
    publish_kernel(backend, kernel).await
}

/// Publishes a prepared kernel as one searchable physical generation.
async fn publish_kernel(backend: &Backend, kernel: Kernel) -> PublishedCommand {
    let lead = kernel
        .chunks()
        .into_iter()
        .find(|chunk| chunk.id == "chunk-0-lead")
        .unwrap();
    kernel
        .database
        .record_disposition(&Disposition {
            revision_id: lead.revision_id,
            outcome: Outcome::Accepted,
            reasons: Vec::new(),
            rule_ids: Vec::new(),
            decided_by: "test".to_owned(),
        })
        .unwrap();
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
    PublishedCommand {
        kernel,
        qdrant,
        generation,
    }
}

/// Searches one literal against the fixture's pinned physical collection.
pub(super) async fn identifier_search(fixture: &PublishedCommand, text: &str) -> RouteOutcome {
    identifier_search_with(fixture, text, 20, None).await
}

/// Searches one literal with the given route limit and exact version filter.
async fn identifier_search_with(
    fixture: &PublishedCommand,
    text: &str,
    limit: usize,
    version: Option<&str>,
) -> RouteOutcome {
    identifier_search_until(
        fixture,
        text,
        limit,
        version,
        Instant::now() + Duration::from_secs(5),
    )
    .await
}

/// Searches one literal with the given limit, version and absolute deadline.
pub(super) async fn identifier_search_until(
    fixture: &PublishedCommand,
    text: &str,
    limit: usize,
    version: Option<&str>,
    deadline: Instant,
) -> RouteOutcome {
    let query = Query {
        generation: &fixture.generation,
        scopes: &fixture.kernel.scopes,
        text,
        limit,
        version,
        qdrant: &fixture.qdrant,
    };
    search_identifiers(
        &query,
        fixture.kernel.database.clone(),
        &understand(text),
        deadline,
    )
    .await
}

#[tokio::test]
async fn empty_scopes_and_zero_limit_skip_both_identifier_legs() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    let understood = understand("ERR-042");
    let empty_scopes = fixture.kernel.database.visible("ungranted-reader").unwrap();
    assert!(empty_scopes.is_empty());
    let filters_before = backend.fake.as_ref().unwrap().scroll_filters().len();

    for (scopes, limit) in [(&empty_scopes, 20), (&fixture.kernel.scopes, 0)] {
        let query = Query {
            generation: &fixture.generation,
            scopes,
            text: "ERR-042",
            limit,
            version: None,
            qdrant: &fixture.qdrant,
        };
        let outcome = search_identifiers(
            &query,
            fixture.kernel.database.clone(),
            &understood,
            Instant::now() + Duration::from_secs(5),
        )
        .await;
        assert_eq!(outcome.status, RouteStatus::Ok);
        assert!(outcome.hits.is_empty());
    }

    assert_eq!(
        backend.fake.as_ref().unwrap().scroll_filters().len(),
        filters_before,
        "empty scopes and zero limit must not start the Qdrant payload leg"
    );
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn identifier_route_accepts_64_and_refuses_65_distinct_values() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    let text = (0..65)
        .map(|index| format!("ERR-{index:03}"))
        .collect::<Vec<_>>()
        .join(" ");
    let understood = understand(&text);
    assert_eq!(understood.identifiers.len(), 65);
    let query = Query {
        generation: &fixture.generation,
        scopes: &fixture.kernel.scopes,
        text: &text,
        limit: 20,
        version: None,
        qdrant: &fixture.qdrant,
    };
    let filters_before = backend.fake.as_ref().unwrap().scroll_filters().len();

    let outcome = search_identifiers(
        &query,
        fixture.kernel.database.clone(),
        &understood,
        Instant::now() + Duration::from_secs(5),
    )
    .await;
    assert_eq!(
        outcome.status,
        RouteStatus::Unavailable("kernel: too many identifier values".to_owned())
    );
    assert!(outcome.hits.is_empty());
    assert_eq!(
        backend.fake.as_ref().unwrap().scroll_filters().len(),
        filters_before,
        "an oversized identifier request must not start the Qdrant payload leg"
    );

    let text = (0..64)
        .map(|index| format!("ERR-{index:03}"))
        .collect::<Vec<_>>()
        .join(" ");
    let understood = understand(&text);
    assert_eq!(understood.identifiers.len(), 64);
    let query = Query {
        generation: &fixture.generation,
        scopes: &fixture.kernel.scopes,
        text: &text,
        limit: 20,
        version: None,
        qdrant: &fixture.qdrant,
    };
    let outcome = search_identifiers(
        &query,
        fixture.kernel.database.clone(),
        &understood,
        Instant::now() + Duration::from_secs(5),
    )
    .await;
    assert_eq!(outcome.status, RouteStatus::Ok);
    assert!(
        backend.fake.as_ref().unwrap().scroll_filters().len() > filters_before,
        "the inclusive 64-value endpoint must reach the payload leg"
    );
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn filtered_scroll_finds_an_allowed_identifier_after_page_one() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    let decoys = (1..=64)
        .map(|id| {
            identifier_point(
                &format!("00000000-0000-0000-0000-{id:012x}"),
                "unowned-duplicate",
                "unowned-revision",
                "workspace/default",
            )
        })
        .collect();
    upsert(&backend, &fixture.generation, decoys).await;

    let outcome = identifier_search(&fixture, "ERR-042").await;
    assert_eq!(outcome.status, RouteStatus::Ok);
    assert_eq!(
        outcome
            .hits
            .iter()
            .filter(|hit| hit.chunk_id == "unowned-duplicate")
            .count(),
        1
    );
    assert_eq!(
        outcome
            .hits
            .iter()
            .filter(|hit| hit.chunk_id == "chunk-0-lead")
            .count(),
        1
    );
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn identifier_limit_applies_after_payload_and_kernel_legs_are_merged() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    upsert(
        &backend,
        &fixture.generation,
        vec![identifier_point(
            "00000000-0000-4000-8000-000000000001",
            "aaa-payload-only",
            "payload-revision",
            "workspace/default",
        )],
    )
    .await;

    let outcome = identifier_search_with(&fixture, "ERR-042", 1, None).await;
    assert_eq!(outcome.status, RouteStatus::Ok);
    assert_eq!(outcome.hits.len(), 1);
    assert_eq!(outcome.hits[0].chunk_id, "aaa-payload-only");
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn payload_leg_collects_distinct_hits_through_its_limit() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    let points = ["payload-only-a", "payload-only-b"]
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
    upsert(&backend, &fixture.generation, points).await;

    let outcome = identifier_search_with(&fixture, "ERR-042", 3, None).await;
    assert_eq!(outcome.status, RouteStatus::Ok);
    assert_eq!(outcome.hits.len(), 3);
    let chunks: Vec<_> = outcome
        .hits
        .iter()
        .map(|hit| hit.chunk_id.as_str())
        .collect();
    assert!(chunks.contains(&"chunk-0-lead"));
    assert!(chunks.contains(&"payload-only-a"));
    assert!(chunks.contains(&"payload-only-b"));
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn forbidden_payload_hits_cannot_displace_the_allowed_identifier_at_limit_one() {
    for backend in
        backends("forbidden_payload_hits_cannot_displace_the_allowed_identifier_at_limit_one")
    {
        let fixture = publish_command(&backend).await;
        let points = [1, 2]
            .into_iter()
            .map(|id| {
                identifier_point(
                    &format!("00000000-0000-0000-0000-{id:012x}"),
                    &format!("aaa-forbidden-{id}"),
                    "forbidden-revision",
                    "workspace/other",
                )
            })
            .collect();
        upsert(&backend, &fixture.generation, points).await;

        let outcome = identifier_search_with(&fixture, "ERR-042", 1, Some("9.0.22")).await;
        assert_eq!(outcome.status, RouteStatus::Ok, "{}", backend.name);
        assert_eq!(outcome.hits.len(), 1, "{}", backend.name);
        assert_eq!(outcome.hits[0].chunk_id, "chunk-0-lead", "{}", backend.name);
        cleanup(&backend, &[&fixture.generation]).await;
    }
}

#[tokio::test]
async fn payload_filter_conjoins_scope_identifier_profile_and_version() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    let outcome = identifier_search_with(&fixture, "ERR-042", 1, Some("9.0.22")).await;
    assert_eq!(outcome.status, RouteStatus::Ok);

    let filters = backend.fake.as_ref().unwrap().scroll_filters();
    let Some(filter) = filters.last() else {
        panic!("identifier search must issue one payload scroll");
    };
    assert_eq!(
        filter.must.iter().map(field_key).collect::<Vec<_>>(),
        ["scope_tags", "version", "identifier_profile", "identifiers"]
    );
    assert!(matches!(
        field_match(filter, "scope_tags"),
        Some(MatchValue::Keywords(values)) if values.strings == ["workspace/default"]
    ));
    assert!(matches!(
        field_match(filter, "version"),
        Some(MatchValue::Keyword(value)) if value == "9.0.22"
    ));
    assert!(matches!(
        field_match(filter, "identifier_profile"),
        Some(MatchValue::Keyword(value)) if value == IDENTIFIER_PROFILE
    ));
    assert!(matches!(
        field_match(filter, "identifiers"),
        Some(MatchValue::Keywords(values)) if values.strings == ["ERR-042"]
    ));
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn too_common_kernel_identifier_is_reported_without_kernel_hits() {
    let backend = fake();
    let kernel = Kernel::with_changed_guides(10, &|kernel, guide, mut chunks| {
        if guide != 0 {
            return chunks;
        }
        for chunk in &mut chunks {
            chunk.digest = kernel.put(b"Install the tool with --force.");
        }
        chunks
    });
    let fixture = publish_kernel(&backend, kernel).await;

    let outcome = identifier_search_with(&fixture, "--force", 20, Some("99.99.99")).await;
    assert_eq!(
        outcome.status,
        RouteStatus::Unavailable("kernel: identifier too common".to_owned())
    );
    assert!(outcome.hits.is_empty());
    cleanup(&backend, &[&fixture.generation]).await;
}

#[tokio::test]
async fn bare_plain_words_are_left_to_lexical_search() {
    for backend in backends("bare_plain_words_are_left_to_lexical_search") {
        let fixture = publish_command(&backend).await;
        let lead = fixture
            .kernel
            .chunks()
            .into_iter()
            .find(|chunk| chunk.id == "chunk-0-lead")
            .unwrap();
        assert_eq!(
            fixture.kernel.input(&lead),
            "The ctm command repairs the local cache at ERR-042."
        );

        let identifier = identifier_search(&fixture, "`ctm`").await;
        assert_eq!(identifier.status, RouteStatus::Ok, "{}", backend.name);
        assert!(identifier.hits.is_empty(), "{}", backend.name);
        let lexical = search_bm25(&Query {
            generation: &fixture.generation,
            scopes: &fixture.kernel.scopes,
            text: "ctm",
            limit: 20,
            version: None,
            qdrant: &fixture.qdrant,
        })
        .await
        .unwrap();
        assert!(
            lexical.iter().any(|hit| hit.chunk_id == "chunk-0-lead"),
            "{}",
            backend.name
        );
        cleanup(&backend, &[&fixture.generation]).await;
    }
}

#[tokio::test]
async fn payload_failure_keeps_kernel_hits_and_no_identifier_skips_qdrant() {
    let backend = fake();
    let fixture = publish_command(&backend).await;
    let fake = backend.fake.as_ref().unwrap();
    fake.refuse_next_n("scroll", Code::Internal, 10);

    let empty = identifier_search(&fixture, "how are you").await;
    assert_eq!(empty.status, RouteStatus::Ok);
    assert!(empty.hits.is_empty());

    let outcome = identifier_search(&fixture, "ERR-042").await;
    assert!(matches!(
        &outcome.status,
        RouteStatus::Unavailable(reason) if reason.starts_with("payload:")
    ));
    assert_eq!(outcome.hits.len(), 1);
    assert_eq!(outcome.hits[0].chunk_id, "chunk-0-lead");
    cleanup(&backend, &[&fixture.generation]).await;
}

fn field_key(condition: &Condition) -> &str {
    let Some(ConditionOneOf::Field(field)) = condition.condition_one_of.as_ref() else {
        panic!("the scroll filter must use field conditions: {condition:?}");
    };
    field.key.as_str()
}

fn field_match<'a>(filter: &'a Filter, key: &str) -> Option<&'a MatchValue> {
    filter.must.iter().find_map(|condition| {
        let Some(ConditionOneOf::Field(field)) = condition.condition_one_of.as_ref() else {
            return None;
        };
        (field.key == key)
            .then_some(field.r#match.as_ref())
            .flatten()
            .and_then(|matched| matched.match_value.as_ref())
    })
}
