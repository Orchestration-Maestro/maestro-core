//! Derived captures require a verified acknowledged parent in the same scope/source.
use super::{n09_profiles::browser, n12_support::Fixture};
use maestro_acquisition::capture::{
    CaptureContext, Captures, Representation, SafeIdentity, Transport,
};
use maestro_kernel::{
    acquisition::{DispatchRequest, Frontier, Handle, LeaseRequest, NewItem, ReceiptError},
    artifact::Digest,
};
use rusqlite::Connection;
use std::{fs, time::Duration};

#[test]
fn n12_parent_must_be_a_capture_and_acknowledged() {
    let mut fixture = Fixture::new();
    let arbitrary = fixture.envelope.inputs;
    derive(&mut fixture, arbitrary, Representation::SelectedHtml);
    assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
    let mut fixture = Fixture::new();
    let prepared = fixture.prepare().unwrap();
    let parent_context = fixture.context.clone();
    derive(&mut fixture, prepared, Representation::SelectedHtml);
    assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
    fixture
        .db
        .acknowledge_capture(&parent_context, prepared)
        .unwrap();
    assert!(fixture.prepare().is_ok());
}
#[test]
fn n12_derivation_matrix_is_closed_with_allowed_neighbours() {
    for (parent_kind, child_kind, allowed) in [
        (Representation::WireBody, Representation::SelectedHtml, true),
        (Representation::WireBody, Representation::ApiRecord, true),
        (
            Representation::RenderedDom,
            Representation::SelectedHtml,
            true,
        ),
        (
            Representation::RenderedDom,
            Representation::ApiRecord,
            false,
        ),
        (
            Representation::SelectedHtml,
            Representation::SelectedHtml,
            true,
        ),
        (
            Representation::SelectedHtml,
            Representation::ApiRecord,
            false,
        ),
        (Representation::ApiRecord, Representation::ApiRecord, true),
        (
            Representation::ApiRecord,
            Representation::SelectedHtml,
            false,
        ),
    ] {
        let mut fixture = if parent_kind == Representation::RenderedDom {
            Fixture::with_policy(browser("browser_render"))
        } else {
            Fixture::new()
        };
        if parent_kind == Representation::RenderedDom {
            fixture.envelope.transport = Transport::BrowserRender;
            fixture.envelope.representation = parent_kind;
        }
        let mut parent = fixture.prepare().unwrap();
        fixture
            .db
            .acknowledge_capture(&fixture.context, parent)
            .unwrap();
        if parent_kind == Representation::SelectedHtml || parent_kind == Representation::ApiRecord {
            derive(&mut fixture, parent, parent_kind);
            parent = fixture.prepare().unwrap();
            fixture
                .db
                .acknowledge_capture(&fixture.context, parent)
                .unwrap();
        }
        derive(&mut fixture, parent, child_kind);
        assert_eq!(
            fixture.prepare().is_ok(),
            allowed,
            "{parent_kind:?} -> {child_kind:?}"
        );
    }
}
#[test]
fn n12_cross_scope_parent_refuses_even_with_the_same_source() {
    let mut fixture = Fixture::new();
    let parent = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, parent)
        .unwrap();
    // Model a substituted scoped evidence handle without changing its bytes.
    let sql = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    sql.execute(
        "UPDATE acquisition_evidence SET scope = 'workspace/default/collection/other'
             WHERE id = ?1",
        [parent.to_string()],
    )
    .unwrap();
    derive(&mut fixture, parent, Representation::SelectedHtml);
    assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
}
#[test]
fn n12_cross_source_parent_refuses_with_the_same_collection_scope() {
    let mut fixture = Fixture::new();
    let other = fixture
        .db
        .lease_source(
            "other",
            &"workspace/default/collection/garden".parse().unwrap(),
            LeaseRequest {
                holder: "worker",
                now: fixture.context.now,
                term: Duration::from_secs(30),
            },
        )
        .unwrap();
    let mut envelope = fixture.envelope.clone();
    envelope.source = "other".into();
    let item = fixture
        .db
        .enqueue(
            &other,
            &NewItem {
                fetch_identity: "https://garden.example/docs/parent".into(),
                authorization_context: envelope.authorization_context.clone(),
                representation_profile: envelope.profile.clone(),
            },
            fixture.context.now,
        )
        .unwrap();
    envelope.item = item.id.to_string().parse().unwrap();
    envelope.requested = SafeIdentity::new(&item.request.fetch_identity).unwrap();
    let lease = fixture
        .db
        .lease(
            &other,
            item.id,
            DispatchRequest {
                lease: LeaseRequest {
                    holder: "worker",
                    now: fixture.context.now,
                    term: Duration::from_secs(30),
                },
                max_attempts: 3,
            },
        )
        .unwrap();
    let context = CaptureContext {
        writer: other,
        item: lease,
        now: fixture.context.now,
    };
    let parent = fixture
        .db
        .prepare_capture(&context, &envelope, b"body", u64::MAX)
        .unwrap()
        .handle;
    fixture.db.acknowledge_capture(&context, parent).unwrap();
    derive(&mut fixture, parent, Representation::SelectedHtml);
    assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
}
#[test]
fn n12_corrupt_parent_is_not_repaired_or_consumed_by_child_prepare() {
    let mut fixture = Fixture::new();
    let parent = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, parent)
        .unwrap();
    let digest = fixture.envelope.artifact.clone();
    let hex = digest.as_str();
    let path = fixture
        .root
        .join("artifacts/sha256")
        .join(hex.get(..2).unwrap())
        .join(hex.get(2..4).unwrap())
        .join(hex);
    fs::write(path, b"xxxx").unwrap(); // Same-size corruption still tests digest refusal.
    derive(&mut fixture, parent, Representation::SelectedHtml);
    assert_eq!(fixture.prepare(), Err(ReceiptError::Storage));
    assert!(fixture.db.get(&digest).is_err());
}
#[test]
fn n12_legacy_body_ack_is_not_an_envelope_parent_ack() {
    let mut fixture = Fixture::new();
    let parent = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge(
            &fixture.context.writer,
            &fixture.context.item,
            &Digest::of(b"body"),
            fixture.context.now,
        )
        .unwrap();
    derive(&mut fixture, parent, Representation::SelectedHtml);
    assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
}
/// A distinct synthetic derived item; no network request or HTML/API interpretation.
fn derive(fixture: &mut Fixture, parent: Handle, kind: Representation) {
    let identity = format!("https://garden.example/docs/child/{parent}");
    let item = fixture
        .db
        .enqueue(
            &fixture.context.writer,
            &NewItem {
                fetch_identity: identity.clone(),
                authorization_context: fixture.envelope.authorization_context.clone(),
                representation_profile: fixture.envelope.profile.clone(),
            },
            fixture.context.now,
        )
        .unwrap();
    fixture.context.item = fixture
        .db
        .lease(
            &fixture.context.writer,
            item.id,
            DispatchRequest {
                lease: LeaseRequest {
                    holder: "worker",
                    now: fixture.context.now,
                    term: Duration::from_secs(30),
                },
                max_attempts: 3,
            },
        )
        .unwrap();
    fixture.envelope.item = item.id.to_string().parse().unwrap();
    fixture.envelope.parent = Some(parent);
    fixture.envelope.representation = kind;
    fixture.envelope.requested = SafeIdentity::new(&identity).unwrap();
    fixture.envelope.final_identity = fixture.envelope.requested.clone();
}

#[test]
fn n12_capture_link_update_and_deletion_are_refused() {
    let fixture = Fixture::new();
    fixture.prepare().unwrap();
    let sql = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    assert!(
        sql.execute(
            "UPDATE acquisition_capture_links SET identity = 'replacement' WHERE item = ?1",
            [fixture.envelope.item.to_string()]
        )
        .is_err()
    );
    assert!(
        sql.execute(
            "DELETE FROM acquisition_capture_links WHERE item = ?1",
            [fixture.envelope.item.to_string()]
        )
        .is_err()
    );
    let count: i64 = sql
        .query_row(
            "SELECT count(*) FROM acquisition_capture_links",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
}
