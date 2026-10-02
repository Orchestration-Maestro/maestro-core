//! N13 review regressions for eligibility, parser ceilings and exclusion coverage.
#![expect(
    clippy::indexing_slicing,
    reason = "authored synthetic policy positions"
)]
use super::{
    n07_parse_url_identity_and_denial_precedence as n07,
    n12_support::Fixture,
    n13_durably_enumerate_public_links_and_bounded_partitions::{
        capture, link_fixture, partition, run, scope,
    },
    support,
};
use maestro_acquisition::{
    CheckedPolicy,
    discovery::{
        links::{DomLinks, LinkExtraction, LinkExtractor},
        partition::discover,
    },
    policy::decision::{RequestKind, admit},
};
use maestro_kernel::{
    acquisition::{Batch, Frontier, NotEnqueued, NotEnqueuedReason, Partitions, ReceiptError},
    artifact::Digest,
};
use serde_json::json;
use std::{future::Future, pin::Pin};

/// Effective denial before the fixture's trusted 1970 clock, not the host clock.
fn denial_policy(effective: &str, unknown: bool) -> CheckedPolicy {
    let (mut collection, mut catalog) = n07::fixture();
    let mut decisions = support::value(&catalog, "decisions");
    decisions["entries"][0]["selector"]["path_prefix"] = "/docs/new".into();
    decisions["entries"][0]["effective_at"] = effective.into();
    decisions["entries"][0]["review_at"] = "2099-02-01T00:00:00Z".into();
    if unknown {
        decisions["entries"][0]["selector"]["object_ids"] = json!(["private-object"]);
    }
    support::put(&mut catalog, "decisions", &decisions);
    let mut wire = support::value(&catalog, "policy");
    wire["sources"][0]["limits"]["dom_bytes"] = 1000.into();
    wire["sources"][0]["limits"]["memory_bytes"] = 1000.into();
    wire["aggregate_limits"] = wire["sources"][0]["limits"].clone();
    support::put(&mut catalog, "policy", &wire);
    support::rebind(&mut collection, &mut catalog);
    n07::checked(collection, &catalog).unwrap()
}

/// Real DOM extraction and durable checkpoint, with no fetch or dispatch effects.
fn discover_body(fixture: &mut Fixture, body: &[u8]) -> Batch {
    let handle = capture(fixture, body);
    run(discover(
        &fixture.db,
        &DomLinks::new().unwrap(),
        (&fixture.context, handle),
        &fixture.policy,
        (partition(), 0),
    ))
    .unwrap()
}

/// The captured parent is not acknowledged while inventory remains uncertain.
fn assert_unacknowledged(fixture: &Fixture) {
    let parent = fixture
        .db
        .page(&fixture.db.visible("reader").unwrap(), "notes", None, 10)
        .unwrap()
        .into_iter()
        .find(|item| item.id == fixture.context.item.item)
        .unwrap();
    assert!(
        parent.capture.is_none(),
        "uncertain discovery acknowledged parent"
    );
}

#[test]
fn n13_review_unknown_selector_holds_coverage() {
    let policy = denial_policy("1970-01-01T00:00:00Z", true);
    let mut request = n07::request("https://garden.example/docs/new", RequestKind::Seed);
    request.now = "1970-01-12T13:46:40Z";
    request.attributes.object_id = Some("public-object");
    assert!(admit(&policy, &request, &n07::Controls::default()).is_ok());
    request.attributes.object_id = None;
    assert!(admit(&policy, &request, &n07::Controls::default()).is_err());
    let mut fixture = Fixture::with_policy(policy);
    let batch = discover_body(&mut fixture, b"<a href='/docs/new'>new</a>");
    assert!(
        !batch.stable,
        "unknown selector metadata is not a proven exclusion"
    );
    assert_eq!(batch.items.len(), 1);
    assert_eq!(batch.expected, Some(1));
    assert!(batch.not_enqueued.is_empty());
    assert_eq!(
        batch.items.first().unwrap().request.fetch_identity,
        "https://garden.example/docs/new"
    );
    let state = fixture
        .db
        .partition(&scope(), batch.partition.id)
        .unwrap()
        .unwrap();
    assert_eq!(state.pending, 1);
    assert_eq!(state.batches, vec![batch.clone()]);
    assert_eq!(
        fixture.db.commit_partition(
            &fixture.context.writer,
            batch.partition.id,
            fixture.context.now
        ),
        Err(ReceiptError::Conflict)
    );
    assert_unacknowledged(&fixture);
}

#[test]
fn n13_review_future_denial_does_not_drop_current_link() {
    // Already effective at N07's usual 2026 clock, but future at the capture clock.
    let policy = denial_policy("2000-01-01T00:00:00Z", false);
    let mut request = n07::request("https://garden.example/docs/new", RequestKind::Seed);
    request.now = "1970-01-12T13:46:40Z";
    assert!(admit(&policy, &request, &n07::Controls::default()).is_ok());
    let mut fixture = Fixture::with_policy(policy);
    let batch = discover_body(&mut fixture, b"<a href='/docs/new'>new</a>");
    assert_eq!(
        batch.items.len(),
        1,
        "currently admitted link must be inventoried"
    );
    assert_eq!(batch.expected, Some(1));
    assert!(batch.stable && !batch.truncated);
    assert!(batch.not_enqueued.is_empty());
}

#[test]
fn n13_review_effective_denial_excludes_current_link() {
    let policy = denial_policy("1970-01-01T00:00:00Z", false);
    let mut request = n07::request("https://garden.example/docs/new", RequestKind::Seed);
    request.now = "1970-01-12T13:46:40Z";
    assert!(admit(&policy, &request, &n07::Controls::default()).is_err());
    let mut fixture = Fixture::with_policy(policy);
    let batch = discover_body(&mut fixture, b"<a href='/docs/new'>new</a>");
    assert!(batch.items.is_empty());
    assert_eq!(batch.expected, Some(0));
    assert_eq!(
        batch.not_enqueued,
        vec![NotEnqueued {
            reference: Digest::of(b"https://garden.example/docs/new"),
            reason: NotEnqueuedReason::PolicyDenial
        }]
    );
    assert!(batch.stable && !batch.truncated);
    fixture
        .db
        .commit_partition(
            &fixture.context.writer,
            batch.partition.id,
            fixture.context.now,
        )
        .unwrap();
}

#[test]
fn n13_review_decoder_memory_cap_precedes_extraction() {
    let policy = super::n09_support::policy_with(|value| {
        value["sources"][0]["limits"]["decode"]["memory_bytes"] = 8.into();
        value["aggregate_limits"] = value["sources"][0]["limits"].clone();
    });
    let mut fixture = Fixture::with_policy(policy);
    let handle = capture(&mut fixture, b"<html><a href='/docs/new'>new</a></html>");
    let batch = run(discover(
        &fixture.db,
        &ForbiddenExtractor,
        (&fixture.context, handle),
        &fixture.policy,
        (partition(), 0),
    ))
    .unwrap();
    assert!(batch.truncated);
    assert!(batch.items.is_empty());
    assert_eq!(
        fixture.db.commit_partition(
            &fixture.context.writer,
            batch.partition.id,
            fixture.context.now
        ),
        Err(ReceiptError::Conflict)
    );
    assert_unacknowledged(&fixture);
}

/// The parser-entry seam must never receive bytes over the decoder memory ceiling.
struct ForbiddenExtractor;
impl LinkExtractor for ForbiddenExtractor {
    fn contract(&self) -> &'static str {
        "synthetic forbidden extractor/1"
    }
    #[expect(
        clippy::panic,
        reason = "forbidden parser entry must fail this regression"
    )]
    fn extract<'a>(
        &'a self,
        _: &'a str,
        _: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<LinkExtraction, ReceiptError>> + Send + 'a>> {
        panic!("decoder memory cap must precede extractor entry");
    }
}

#[test]
fn n13_review_non_fetch_schemes_are_exclusions() {
    for url in [
        "mailto:help@example.test",
        "tel:+123456",
        "javascript:void(0)",
        "data:text/plain,hello",
        "ftp://example.test/file",
    ] {
        let mut fixture = link_fixture();
        let batch = discover_body(
            &mut fixture,
            format!("<a href='{url}'>contact</a>").as_bytes(),
        );
        assert!(
            batch.stable && !batch.truncated,
            "non-fetch scheme held coverage: {url}"
        );
        assert!(batch.items.is_empty());
        assert_eq!(batch.expected, Some(0));
        assert_eq!(
            batch.not_enqueued,
            vec![NotEnqueued {
                reference: Digest::of(url.as_bytes()),
                reason: NotEnqueuedReason::NonFetchScheme
            }]
        );
        fixture
            .db
            .commit_partition(
                &fixture.context.writer,
                batch.partition.id,
                fixture.context.now,
            )
            .unwrap();
    }
}

#[test]
fn n13_review_malformed_reference_still_holds() {
    let mut fixture = link_fixture();
    let batch = discover_body(&mut fixture, b"<a href='https://[broken'>broken</a>");
    assert!(batch.truncated || !batch.stable);
    assert_eq!(
        fixture.db.commit_partition(
            &fixture.context.writer,
            batch.partition.id,
            fixture.context.now
        ),
        Err(ReceiptError::Conflict)
    );
    assert_unacknowledged(&fixture);
}
