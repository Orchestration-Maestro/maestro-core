//! Durable resume never promotes derivatives of a failed prerequisite.
use super::{n12_parents::derive, n12_support::Fixture};
use maestro_kernel::acquisition::{Captures, Frontier, Receipts, Representation};
use rusqlite::Connection;
use std::slice;

#[test]
fn n37_failed_prerequisite_fences_acknowledged_stale_derivative() {
    use maestro_kernel::acquisition::{CaptureContext, DispatchRequest, LeaseRequest};
    use std::time::Duration;
    let mut fixture = Fixture::new();
    let parent_context = fixture.context.clone();
    let parent_envelope = fixture.envelope.clone();
    let parent = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&parent_context, parent)
        .unwrap();
    derive(&mut fixture, parent, Representation::SelectedHtml);
    let child = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, child)
        .unwrap();
    let scope = "workspace/default/collection/garden".parse().unwrap();
    let scopes = fixture.db.visible("reader").unwrap();
    let item = Frontier::page(&fixture.db, &scopes, "notes", None, 100)
        .unwrap()
        .into_iter()
        .find(|row| row.id == fixture.context.item.item)
        .unwrap();
    assert_eq!(fixture.db.capture_for(&scope, &item).unwrap(), Some(child));
    fixture
        .db
        .refresh(
            &parent_context.writer,
            parent_context.item.item,
            parent_context.now,
        )
        .unwrap();
    assert!(
        fixture.db.verify_capture(&scope, &item, child).is_err(),
        "stale derivative verified after prerequisite refresh"
    );
    assert!(
        fixture.db.read_capture(&fixture.context, child, 4).is_err(),
        "dependent stage consumed stale prepared bytes"
    );
    assert_eq!(fixture.db.capture_for(&scope, &item).unwrap(), None);
    assert!(
        fixture
            .db
            .capture_page(&scope, slice::from_ref(&item))
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture.db.read("reader", child).unwrap().is_some(),
        "immutable history removed"
    );
    let fresh = CaptureContext {
        writer: parent_context.writer.clone(),
        item: fixture
            .db
            .lease(
                &parent_context.writer,
                parent_context.item.item,
                DispatchRequest {
                    lease: LeaseRequest {
                        holder: "worker",
                        now: parent_context.now,
                        term: Duration::from_secs(30),
                    },
                    max_attempts: 3,
                },
            )
            .unwrap(),
        now: parent_context.now,
    };
    let equal = fixture
        .db
        .prepare_capture(&fresh, &parent_envelope, b"body", u64::MAX)
        .unwrap()
        .handle;
    fixture.db.acknowledge_capture(&fresh, equal).unwrap();
    assert!(
        fixture.db.verify_capture(&scope, &item, child).is_err(),
        "equal bytes from a newer parent generation excused stale derivative"
    );
}

#[test]
fn n37_cancel_releases_owned_frontier_children_without_losing_attempts() {
    let fixture = Fixture::new();
    fixture
        .db
        .release_source(&fixture.context.writer, fixture.context.now)
        .unwrap();
    let sql = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    let (expires, attempts): (String, i64) = sql
        .query_row(
            "SELECT lease_expires, attempts FROM acquisition_frontier WHERE id = ?1",
            [fixture.context.item.item.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert!(
        expires.as_str() <= "1970-01-12T13:46:40.000Z",
        "cancel left owned dispatch live"
    );
    assert_eq!(attempts, 1);
    assert!(
        fixture
            .db
            .acknowledge_capture(
                &fixture.context,
                fixture.prepare().unwrap_or(fixture.envelope.inputs)
            )
            .is_err()
    );
    let rows = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        100,
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows.first().unwrap().capture.is_none());
}

#[test]
fn n37_expired_leases_and_stale_epochs_cannot_acknowledge_resumed_work() {
    use super::n37_support::{item, redispatch, scope};
    let mut fixture = Fixture::new();
    let capture = fixture.prepare().unwrap();
    let stale = fixture.context.clone();
    redispatch(&mut fixture);
    assert!(fixture.db.read_capture(&stale, capture, 4).is_err());
    assert!(fixture.db.acknowledge_capture(&stale, capture).is_err());
    assert!(fixture.context.writer.epoch > stale.writer.epoch);
    assert!(fixture.context.item.epoch > stale.item.epoch);
    fixture
        .db
        .acknowledge_capture(&fixture.context, capture)
        .unwrap();
    let row = item(&fixture);
    assert_eq!(row.attempts, 2);
    assert_eq!(
        fixture.db.capture_for(&scope(), &row).unwrap(),
        Some(capture)
    );
    fixture
        .db
        .acknowledge_capture(&fixture.context, capture)
        .unwrap();
    assert_eq!(item(&fixture).attempts, 2, "replay dispatched again");
    let sql = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    let count: i64 = sql
        .query_row(
            "SELECT count(*) FROM acquisition_capture_links",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "resume created duplicate occurrence");
}

#[test]
fn n37_current_policy_authority_and_context_reapply_to_prepared_and_completed() {
    use super::{
        n07_parse_url_identity_and_denial_precedence::Controls,
        n09_support::{Grants, policy_with},
        n37_support::{current, item},
    };
    use maestro_acquisition::lifecycle::resume::{completed, prepared};
    use maestro_kernel::artifact::Digest;
    let fixture = Fixture::new();
    let capture = fixture.prepare().unwrap();
    let controls = Controls::default();
    let grants = Grants::default();
    let row = item(&fixture);
    let access = current(&fixture, &fixture.policy, &controls, &grants);
    assert_eq!(
        prepared(&fixture.db, &access, (&row, &fixture.context), capture, 4)
            .unwrap()
            .1,
        Some(b"body".to_vec())
    );
    assert_eq!(
        completed(&fixture.db, &access, &row).unwrap(),
        None,
        "prepared is not completed"
    );
    fixture
        .db
        .acknowledge_capture(&fixture.context, capture)
        .unwrap();
    let row = item(&fixture);
    assert_eq!(
        completed(&fixture.db, &access, &row).unwrap(),
        Some(capture)
    );
    let tightened = policy_with(|value| {
        for selector in value["sources"][0]["selectors"].as_array_mut().unwrap() {
            selector["path_prefix"] = "/docs/tightened".into();
        }
    });
    let tightened = current(&fixture, &tightened, &controls, &grants);
    assert!(completed(&fixture.db, &tightened, &row).is_err());
    assert!(
        prepared(
            &fixture.db,
            &tightened,
            (&row, &fixture.context),
            capture,
            4
        )
        .is_err()
    );
    grants.refuse_after.set(grants.calls.get());
    assert!(
        completed(&fixture.db, &access, &row).is_err(),
        "revoked authority reused bytes"
    );
    assert!(prepared(&fixture.db, &access, (&row, &fixture.context), capture, 4).is_err());
    grants.refuse_after.set(0);
    for field in 0..2 {
        let mut changed = row.clone();
        if field == 0 {
            changed.request.authorization_context = Digest::of(b"different account");
        } else {
            changed.request.representation_profile = Digest::of(b"different representation");
        }
        assert!(
            access.check(&changed).is_err(),
            "changed context {field} reused"
        );
    }
}

#[test]
fn n37_cancel_finalizes_existing_receipt_and_preserves_pending_inventory() {
    use super::n37_support::{item, receipt, scope};
    use maestro_acquisition::capture::{Cancellation, cancel_run};
    use maestro_kernel::acquisition::{
        Handle, InventoryPage, InventorySchema, ItemDisposition, LeaseRequest, Stage, StageItem,
        Status,
    };
    use std::time::Duration;
    let mut fixture = Fixture::new();
    let receipt = receipt(&mut fixture);
    let partition = fixture
        .db
        .retain(&scope(), b"pending partition", &[])
        .unwrap();
    let frontier = vec![item(&fixture)];
    let pages = vec![InventoryPage {
        schema: InventorySchema::V1,
        partition,
        stage: Stage::Capture,
        complete: false,
        items: vec![StageItem {
            item: fixture.envelope.item,
            disposition: ItemDisposition::Pending,
            evidence: None,
        }],
    }];
    let cancellation = Cancellation {
        writer: &fixture.context.writer,
        now: fixture.context.now,
        scope: &scope(),
        receipt: &receipt,
        pages: &pages,
        frontier: &frontier,
    };
    let outcome = cancel_run(&fixture.db, &cancellation).unwrap();
    assert_eq!(outcome.status, Status::Cancelled);
    let terminal = fixture
        .db
        .inspect("reader", receipt.attempt)
        .unwrap()
        .unwrap();
    assert_eq!(terminal.status, Status::Cancelled);
    assert_eq!(terminal.inputs, receipt.inputs);
    assert_eq!(terminal.run, receipt.run);
    assert_eq!(terminal.attempt, receipt.attempt);
    assert_eq!(terminal.attempts, 2);
    fixture
        .db
        .lease_source(
            "notes",
            &scope(),
            LeaseRequest {
                holder: "next-run",
                now: fixture.context.now,
                term: Duration::from_secs(30),
            },
        )
        .unwrap();
    assert_eq!(terminal.inventories.len(), 1);
    let retained = fixture
        .db
        .read("reader", *terminal.inventories.first().unwrap())
        .unwrap()
        .unwrap();
    let inventory: InventoryPage = serde_json::from_slice(retained.bytes()).unwrap();
    assert_eq!(inventory, *pages.first().unwrap());
    assert!(item(&fixture).capture.is_none());
    assert!(
        cancel_run(&fixture.db, &cancellation).is_err(),
        "cancel replay released a successor"
    );
    assert!(
        fixture
            .db
            .inspect("reader", Handle::new())
            .unwrap()
            .is_none()
    );
}

#[test]
fn n37_transitive_prerequisite_failure_holds_grandchild() {
    use super::n37_support::{item, scope};
    let mut fixture = Fixture::new();
    let original_context = fixture.context.clone();
    let parent = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, parent)
        .unwrap();
    derive(&mut fixture, parent, Representation::SelectedHtml);
    let child = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, child)
        .unwrap();
    derive(&mut fixture, child, Representation::SelectedHtml);
    let grandchild = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, grandchild)
        .unwrap();
    assert_eq!(
        fixture.db.capture_for(&scope(), &item(&fixture)).unwrap(),
        Some(grandchild)
    );
    fixture
        .db
        .refresh(
            &original_context.writer,
            original_context.item.item,
            original_context.now,
        )
        .unwrap();
    assert_eq!(
        fixture.db.capture_for(&scope(), &item(&fixture)).unwrap(),
        None
    );
    assert!(
        fixture
            .db
            .read_capture(&fixture.context, grandchild, 4)
            .is_err()
    );
}

#[test]
fn n37_prepared_refuses_mismatched_item_scope_and_frozen_inputs() {
    use super::{
        n07_parse_url_identity_and_denial_precedence::Controls,
        n09_support::{Grants, policy_with},
        n37_support::{current, item, scope},
    };
    use maestro_acquisition::lifecycle::resume::prepared;
    let fixture = Fixture::new();
    let capture = fixture.prepare().unwrap();
    let controls = Controls::default();
    let grants = Grants::default();
    let row = item(&fixture);
    let mut access = current(&fixture, &fixture.policy, &controls, &grants);
    let mut wrong = fixture.context.clone();
    wrong.item.item = "00000000000000000000000000".parse().unwrap();
    assert!(prepared(&fixture.db, &access, (&row, &wrong), capture, 4).is_err());
    let extra_source = policy_with(|value| {
        let mut source = value["sources"][0].clone();
        source["id"] = "other".into();
        for selector in source["selectors"].as_array_mut().unwrap() {
            selector["source_id"] = "other".into();
        }
        value["sources"].as_array_mut().unwrap().push(source);
    });
    let mut other_item = row.clone();
    other_item.source = "other".into();
    let other_access = current(&fixture, &extra_source, &controls, &grants);
    assert!(other_access.check(&other_item).is_ok());
    assert!(
        prepared(
            &fixture.db,
            &other_access,
            (&other_item, &fixture.context),
            capture,
            4
        )
        .is_err()
    );
    let other = "workspace/default/collection/other".parse().unwrap();
    access.scope = &other;
    assert!(prepared(&fixture.db, &access, (&row, &fixture.context), capture, 4).is_err());
    let original_scope = scope();
    access.scope = &original_scope;
    access.inputs = fixture
        .db
        .retain(&scope(), b"changed frozen policy", &[])
        .unwrap();
    assert!(
        prepared(&fixture.db, &access, (&row, &fixture.context), capture, 4).is_err(),
        "changed frozen inputs adopted"
    );
}
