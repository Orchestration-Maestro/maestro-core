//! N13 durable synthetic discovery and partition contracts.
#![expect(
    clippy::indexing_slicing,
    reason = "authored synthetic policy positions"
)]
use super::n12_support::Fixture;
use maestro_acquisition::discovery::{
    links::{LinkExtraction, LinkExtractor},
    partition::discover,
};
use maestro_kernel::{
    acquisition::{
        Batch, Captures, ChangeKeys, DiscoveredItem, Enumeration, Frontier, Handle, NewItem,
        Partition, Partitions, ReceiptError, Window,
    },
    artifact::Digest,
    scope::Scope,
    store::Database,
};
use rusqlite::Connection;
use serde_json::json;
use std::{future::Future, pin::Pin};

/// Frozen finite window with explicit overlap/skew, never a search query.
pub(super) fn partition() -> Partition {
    Partition {
        id: Handle::new(),
        run: Handle::new(),
        kind: Enumeration::Index,
        window: Window {
            start: 10,
            end: 20,
            overlap: 2,
            skew: 1,
        },
        max_batches: 8,
        max_items: 20,
    }
}
/// Exact source/context, with every non-text change dimension retained.
pub(super) fn item(fixture: &Fixture, suffix: &str) -> DiscoveredItem {
    DiscoveredItem {
        request: NewItem {
            fetch_identity: format!("https://garden.example/docs/{suffix}"),
            authorization_context: fixture.envelope.authorization_context.clone(),
            representation_profile: fixture.envelope.profile.clone(),
        },
        keys: ChangeKeys {
            revision: None,
            validator: Some(Digest::of(
                &serde_json::to_vec(&fixture.envelope.headers).unwrap(),
            )),
            metadata: Some(Digest::of(b"metadata")),
            permissions: fixture.envelope.authorization_context.clone(),
            links: Digest::of(suffix.as_bytes()),
            representation: fixture.envelope.artifact.clone(),
        },
    }
}
/// A terminal complete batch; tests remove individual evidence below.
pub(super) fn batch(fixture: &Fixture) -> Batch {
    Batch {
        partition: partition(),
        cursor: None,
        next: None,
        terminal: true,
        stable: true,
        truncated: false,
        expected: Some(1),
        items: vec![item(fixture, "new")],
        capture: None,
        extractor: None,
        denied: vec![],
    }
}
/// Real scoped protected capture, prepared without acknowledgment.
pub(super) fn capture(fixture: &mut Fixture, body: &[u8]) -> Handle {
    fixture.envelope.artifact = Digest::of(body);
    fixture.envelope.length = body.len() as u64;
    fixture.envelope.declared_media = Some("text/html".into());
    fixture
        .db
        .prepare_capture(&fixture.context, &fixture.envelope, body, u64::MAX)
        .unwrap()
        .handle
}

#[test]
fn n13_links_change_without_visible_text_and_survive_restart() {
    for target in ["one", "two"] {
        let mut fixture = link_fixture();
        let body = format!("<html><body><a href='/docs/{target}'>same text</a></body></html>");
        let handle = capture(&mut fixture, body.as_bytes());
        let result = run(async {
            discover(
                &fixture.db,
                &FakeLinks::new(&[target]),
                (&fixture.context, handle),
                &fixture.policy,
                partition(),
            )
            .await
        })
        .unwrap();
        assert_eq!(result.items.len(), 1);
        assert_eq!(
            result.items.first().unwrap().request.fetch_identity,
            format!("https://garden.example/docs/{target}")
        );
        let reopened = Database::open_in(&fixture.root).unwrap();
        let saved = reopened
            .partition(&scope(), result.partition.id)
            .unwrap()
            .unwrap();
        assert_eq!(saved.batches, vec![result]);
        assert!(saved.accepted.is_none());
        assert_eq!(
            reopened
                .page(&reopened.visible("reader").unwrap(), "notes", None, 10)
                .unwrap()
                .len(),
            2
        );
    }
}
#[test]
fn n13_one_failed_enqueue_never_acknowledges_discovery() {
    let mut fixture = link_fixture();
    let handle = capture(
        &mut fixture,
        b"<html><a href='/docs/one'>one</a><a href='/docs/two'>two</a></html>",
    );
    Connection::open(fixture.root.join("kernel.sqlite3"))
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER fail_child BEFORE INSERT ON acquisition_frontier WHEN
        NEW.fetch_identity LIKE '%/two' BEGIN SELECT RAISE(ABORT, 'synthetic
        failure'); END;",
        )
        .unwrap();
    let result = run(async {
        discover(
            &fixture.db,
            &FakeLinks::new(&["one", "two"]),
            (&fixture.context, handle),
            &fixture.policy,
            partition(),
        )
        .await
    });
    assert!(result.is_err());
    let parent = fixture
        .db
        .page(&fixture.db.visible("reader").unwrap(), "notes", None, 10)
        .unwrap()
        .into_iter()
        .find(|item| item.id == fixture.context.item.item)
        .unwrap();
    assert!(
        parent.capture.is_none(),
        "enqueue must precede acknowledgment"
    );
}
#[test]
fn n13_truncated_or_unstable_partition_never_completes() {
    for (stable, truncated) in [(false, false), (true, true)] {
        let fixture = Fixture::new();
        let mut batch = batch(&fixture);
        batch.items.clear();
        batch.expected = Some(0);
        batch.stable = stable;
        batch.truncated = truncated;
        fixture
            .db
            .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
            .unwrap();
        assert_eq!(
            fixture.db.commit_partition(
                &fixture.context.writer,
                batch.partition.id,
                fixture.context.now
            ),
            Err(ReceiptError::Conflict)
        );
        let saved = fixture
            .db
            .partition(&scope(), batch.partition.id)
            .unwrap()
            .unwrap();
        assert_eq!(saved.batches, vec![batch]);
        assert!(saved.accepted.is_none());
    }
}
#[test]
fn n13_empty_nonterminal_batch_and_replayed_cursor_remain_pending() {
    let fixture = Fixture::new();
    let mut batch = batch(&fixture);
    batch.items.clear();
    batch.expected = Some(0);
    batch.terminal = false;
    batch.next = Some(json!({"offset": 1, "token": ["opaque", 5]}));
    fixture
        .db
        .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
        .unwrap();
    fixture
        .db
        .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
        .unwrap();
    assert_eq!(
        fixture.db.commit_partition(
            &fixture.context.writer,
            batch.partition.id,
            fixture.context.now
        ),
        Err(ReceiptError::Conflict)
    );
    let mut replay = batch.clone();
    replay.cursor = batch.next.clone();
    replay.next = batch.next.clone();
    assert_eq!(
        fixture
            .db
            .checkpoint(&fixture.context.writer, &replay, fixture.context.now),
        Err(ReceiptError::Conflict)
    );
    replay.cursor = None;
    replay.next = Some(json!({"offset": 2}));
    assert_eq!(
        fixture
            .db
            .checkpoint(&fixture.context.writer, &replay, fixture.context.now),
        Err(ReceiptError::Conflict)
    );
    assert_eq!(
        fixture
            .db
            .partition(&scope(), batch.partition.id)
            .unwrap()
            .unwrap()
            .batches,
        vec![batch]
    );
}
#[test]
fn n13_failed_item_holds_watermark_until_verified_commit() {
    let fixture = Fixture::new();
    let handle = fixture.prepare().unwrap();
    let mut batch = batch(&fixture);
    batch.items = vec![DiscoveredItem {
        request: fixture
            .db
            .page(&fixture.db.visible("reader").unwrap(), "notes", None, 10)
            .unwrap()
            .remove(0)
            .request,
        ..item(&fixture, "start")
    }];
    fixture
        .db
        .checkpoint(&fixture.context.writer, &batch, fixture.context.now)
        .unwrap();
    assert_eq!(
        fixture.db.commit_partition(
            &fixture.context.writer,
            batch.partition.id,
            fixture.context.now
        ),
        Err(ReceiptError::Conflict)
    );
    assert!(
        fixture
            .db
            .partition(&scope(), batch.partition.id)
            .unwrap()
            .unwrap()
            .accepted
            .is_none()
    );
    fixture
        .db
        .acknowledge_capture(&fixture.context, handle)
        .unwrap();
    fixture
        .db
        .commit_partition(
            &fixture.context.writer,
            batch.partition.id,
            fixture.context.now,
        )
        .unwrap();
    let saved = fixture
        .db
        .partition(&scope(), batch.partition.id)
        .unwrap()
        .unwrap();
    assert_eq!(saved.pending, 0);
    assert_eq!(saved.accepted.unwrap().watermark, 20);
    assert_eq!(saved.batches, vec![batch]);
}
/// Same collection as N12; other scopes must never see these records.
pub(super) fn scope() -> Scope {
    "workspace/default/collection/garden".parse().unwrap()
}

/// Existing synthetic Tokio boundary with a returned result.
pub(super) fn run<T>(work: impl Future<Output = T>) -> T {
    let mut result = None;
    super::n09_support::run(async {
        result = Some(work.await);
    });
    result.unwrap()
}

/// Generous but explicit source bounds for a synthetic link inventory.
pub(super) fn link_fixture() -> Fixture {
    Fixture::with_policy(super::n09_support::policy_with(|value| {
        value["sources"][0]["limits"]["pages"] = 20.into();
        value["sources"][0]["limits"]["partitions"] = 8.into();
        value["aggregate_limits"] = value["sources"][0]["limits"].clone();
    }))
}

#[test]
fn n13_hidden_href_change_revises_raw_representation() {
    let mut keys = vec![];
    for hidden in ["one", "two"] {
        let mut fixture = link_fixture();
        let body = format!(
            "<html><a href='/docs/new'>same</a><a aria-hidden='true'
                href='/docs/{hidden}'>same</a></html>"
        );
        let handle = capture(&mut fixture, body.as_bytes());
        let batch = run(discover(
            &fixture.db,
            &FakeLinks::new(&["new"]),
            (&fixture.context, handle),
            &fixture.policy,
            partition(),
        ))
        .unwrap();
        keys.push(batch.items.first().unwrap().keys.clone());
    }
    assert_eq!(keys.first().unwrap().links, keys.last().unwrap().links);
    assert_ne!(
        keys.first().unwrap().representation,
        keys.last().unwrap().representation
    );
}
#[test]
fn n13_byte_cap_precedes_extraction_and_preserves_pending_checkpoint() {
    let policy = super::n09_support::policy_with(|value| {
        value["sources"][0]["limits"]["dom_bytes"] = 8.into();
        value["aggregate_limits"] = value["sources"][0]["limits"].clone();
    });
    let mut fixture = Fixture::with_policy(policy);
    let handle = capture(&mut fixture, b"<html><a href='/docs/new'>new</a></html>");
    let batch = run(discover(
        &fixture.db,
        &NeverCalled,
        (&fixture.context, handle),
        &fixture.policy,
        partition(),
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
    let items = fixture
        .db
        .page(&fixture.db.visible("reader").unwrap(), "notes", None, 10)
        .unwrap();
    assert!(items.first().unwrap().capture.is_none());
}
/// A real test seam proves no parser entry when captured HTML exceeds its ceiling.
struct NeverCalled;
impl LinkExtractor for NeverCalled {
    fn contract(&self) -> &'static str {
        "synthetic disabled extractor"
    }
    fn extract<'a>(
        &'a self,
        _: &'a str,
        _: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<LinkExtraction, ReceiptError>> + Send + 'a>> {
        assert!(bytes_are_bounded(), "HTML cap must precede extraction");
        Box::pin(async { Err(ReceiptError::Invalid) })
    }
}
#[test]
fn n13_item_cap_and_unknown_coverage_cannot_advance_watermark() {
    let mut fixture = Fixture::new();
    let handle = capture(
        &mut fixture,
        b"<html><a href='/docs/one'>one</a><a href='/docs/two'>two</a></html>",
    );
    let batch = run(discover(
        &fixture.db,
        &FakeLinks::new(&["one", "two"]),
        (&fixture.context, handle),
        &fixture.policy,
        partition(),
    ))
    .unwrap();
    assert!(batch.truncated);
    assert_eq!(batch.expected, Some(2));
    assert_eq!(batch.items.len(), 1);
    assert!(
        fixture
            .db
            .page(&fixture.db.visible("reader").unwrap(), "notes", None, 10)
            .unwrap()
            .iter()
            .all(|item| item.capture.is_none())
    );
    let mut unknown = batch.clone();
    unknown.partition.id = Handle::new();
    unknown.truncated = false;
    unknown.expected = None;
    fixture
        .db
        .checkpoint(&fixture.context.writer, &unknown, fixture.context.now)
        .unwrap();
    assert_eq!(
        fixture.db.commit_partition(
            &fixture.context.writer,
            unknown.partition.id,
            fixture.context.now
        ),
        Err(ReceiptError::Conflict)
    );
}
#[test]
fn n13_complete_typed_cursor_chain_commits_empty_terminal_window() {
    let fixture = Fixture::new();
    let mut first = batch(&fixture);
    first.items.clear();
    first.expected = Some(0);
    first.terminal = false;
    first.next = Some(json!({"token": ["one", {"sequence": 2}]}));
    fixture
        .db
        .checkpoint(&fixture.context.writer, &first, fixture.context.now)
        .unwrap();
    let mut terminal = first.clone();
    terminal.cursor = first.next.clone();
    terminal.next = None;
    terminal.terminal = true;
    fixture
        .db
        .checkpoint(&fixture.context.writer, &terminal, fixture.context.now)
        .unwrap();
    fixture
        .db
        .commit_partition(
            &fixture.context.writer,
            terminal.partition.id,
            fixture.context.now,
        )
        .unwrap();
    let reopened = Database::open_in(&fixture.root).unwrap();
    let state = reopened
        .partition(&scope(), terminal.partition.id)
        .unwrap()
        .unwrap();
    assert_eq!(state.batches, vec![first, terminal]);
    assert_eq!(state.accepted.unwrap().watermark, 20);
}

/// Offline substitute seam; authored candidates do not parse or dispatch.
struct FakeLinks(Vec<String>);
impl FakeLinks {
    fn new(paths: &[&str]) -> Self {
        Self(
            paths
                .iter()
                .map(|path| format!("https://garden.example/docs/{path}"))
                .collect(),
        )
    }
}
impl LinkExtractor for FakeLinks {
    fn contract(&self) -> &'static str {
        "synthetic offline extractor/1"
    }
    fn extract<'a>(
        &'a self,
        _: &'a str,
        _: &'a [u8],
    ) -> Pin<Box<dyn Future<Output = Result<LinkExtraction, ReceiptError>> + Send + 'a>> {
        Box::pin(async {
            Ok(LinkExtraction {
                links: self.0.clone(),
                contract: self.contract().into(),
                limit_hit: false,
            })
        })
    }
}

/// A disabled test parser never has admitted bytes.
fn bytes_are_bounded() -> bool {
    false
}
