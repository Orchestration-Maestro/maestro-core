//! Cancellation ordering and prepared/item epoch fences.
use super::n12_support::Fixture;
use maestro_kernel::acquisition::{Captures, Frontier, Receipts};

#[test]
fn n37_invalid_cancel_inventory_refuses_before_release_and_remains_recoverable() {
    use super::n37_support::{item, receipt, scope};
    use maestro_acquisition::capture::{Cancellation, cancel_run};
    use maestro_kernel::acquisition::{
        InventoryPage, InventorySchema, ItemDisposition, LeaseRequest, Stage, StageItem, Status,
        UnfinalizedPage,
    };
    use std::time::Duration;
    let mut fixture = Fixture::new();
    let receipt = receipt(&mut fixture);
    let frontier = vec![item(&fixture)];
    let pages = vec![InventoryPage {
        schema: InventorySchema::V1,
        partition: fixture.envelope.inputs,
        stage: Stage::Capture,
        complete: true,
        items: vec![StageItem {
            item: fixture.envelope.item,
            disposition: ItemDisposition::Accepted,
            evidence: Some(fixture.envelope.inputs),
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
    assert!(cancel_run(&fixture.db, &cancellation).is_err());
    assert!(
        fixture
            .db
            .lease_source(
                "notes",
                &scope(),
                LeaseRequest {
                    holder: "contender",
                    now: fixture.context.now,
                    term: Duration::from_secs(30)
                }
            )
            .is_err(),
        "invalid cancellation released ownership"
    );
    let recovered = fixture
        .db
        .unfinalized(
            "reader",
            &UnfinalizedPage {
                scope: &scope(),
                now: fixture.context.now + Duration::from_secs(31),
                after: None,
                limit: 100,
            },
        )
        .unwrap();
    assert_eq!(recovered.len(), 1);
    assert_eq!(recovered.first().unwrap().attempt, receipt.attempt);
    assert_eq!(recovered.first().unwrap().status, Status::Pending);
}

#[test]
fn n37_item_epoch_refuses_stale_dispatch_under_same_live_source() {
    use super::n37_support::{item, scope};
    use maestro_kernel::acquisition::{
        CaptureContext, DispatchRequest, LeaseRequest, SafeIdentity,
    };
    use std::time::Duration;
    let fixture = Fixture::new();
    let mut request = item(&fixture).request;
    request.fetch_identity = "https://garden.example/docs/child".into();
    let row = fixture
        .db
        .enqueue(&fixture.context.writer, &request, fixture.context.now)
        .unwrap();
    let lease = LeaseRequest {
        holder: "worker",
        now: fixture.context.now,
        term: Duration::from_secs(1),
    };
    let old = CaptureContext {
        writer: fixture.context.writer.clone(),
        item: fixture
            .db
            .lease(
                &fixture.context.writer,
                row.id,
                DispatchRequest {
                    lease,
                    max_attempts: 2,
                },
            )
            .unwrap(),
        now: fixture.context.now,
    };
    let mut envelope = fixture.envelope.clone();
    envelope.item = row.id.to_string().parse().unwrap();
    envelope.requested = SafeIdentity::new(&request.fetch_identity).unwrap();
    envelope.final_identity = envelope.requested.clone();
    let capture = fixture
        .db
        .prepare_capture(&old, &envelope, b"body", u64::MAX)
        .unwrap()
        .handle;
    let mut expired = old.clone();
    expired.now += Duration::from_secs(2);
    assert!(fixture.db.read_capture(&expired, capture, 4).is_err());
    let fresh = CaptureContext {
        writer: old.writer.clone(),
        item: fixture
            .db
            .lease(
                &old.writer,
                row.id,
                DispatchRequest {
                    lease: LeaseRequest {
                        holder: "worker",
                        now: expired.now,
                        term: Duration::from_secs(5),
                    },
                    max_attempts: 2,
                },
            )
            .unwrap(),
        now: expired.now,
    };
    assert_eq!(fresh.writer.epoch, old.writer.epoch);
    assert!(fresh.item.epoch > old.item.epoch);
    assert!(
        fixture.db.acknowledge_capture(&expired, capture).is_err(),
        "old item epoch accepted by live source"
    );
    fixture.db.acknowledge_capture(&fresh, capture).unwrap();
    assert!(
        fixture
            .db
            .capture_for(
                &scope(),
                &Frontier::page(
                    &fixture.db,
                    &fixture.db.visible("reader").unwrap(),
                    "notes",
                    None,
                    100
                )
                .unwrap()
                .into_iter()
                .find(|item| item.id == row.id)
                .unwrap()
            )
            .unwrap()
            .is_some()
    );
}

#[test]
fn n37_pending_prepared_refresh_requires_idle_current_epoch() {
    use super::n37_support::{item, scope};
    use maestro_kernel::acquisition::{DispatchRequest, LeaseRequest};
    use std::time::Duration;
    let mut fixture = Fixture::new();
    let original = fixture.prepare().unwrap();
    assert!(
        fixture
            .db
            .refresh(
                &fixture.context.writer,
                fixture.context.item.item,
                fixture.context.now
            )
            .is_err(),
        "live item lease stolen"
    );
    assert_eq!(
        fixture.db.prepared_for(&scope(), &item(&fixture)).unwrap(),
        Some(original)
    );
    let stale = fixture.context.clone();
    fixture
        .db
        .release_source(&fixture.context.writer, fixture.context.now)
        .unwrap();
    fixture.context.writer = fixture
        .db
        .lease_source(
            "notes",
            &scope(),
            LeaseRequest {
                holder: "worker",
                now: fixture.context.now,
                term: Duration::from_secs(30),
            },
        )
        .unwrap();
    assert!(
        fixture
            .db
            .refresh(&stale.writer, stale.item.item, stale.now)
            .is_err()
    );
    fixture
        .db
        .refresh(
            &fixture.context.writer,
            fixture.context.item.item,
            fixture.context.now,
        )
        .unwrap();
    assert_eq!(
        fixture.db.prepared_for(&scope(), &item(&fixture)).unwrap(),
        None
    );
    assert!(
        fixture
            .db
            .refresh(
                &fixture.context.writer,
                fixture.context.item.item,
                fixture.context.now
            )
            .is_err(),
        "empty pending row repeatedly refreshed"
    );
    fixture.context.item = fixture
        .db
        .lease(
            &fixture.context.writer,
            fixture.context.item.item,
            DispatchRequest {
                lease: LeaseRequest {
                    holder: "resumed",
                    now: fixture.context.now,
                    term: Duration::from_secs(30),
                },
                max_attempts: 3,
            },
        )
        .unwrap();
    assert!(
        fixture
            .db
            .acknowledge_capture(&fixture.context, original)
            .is_err(),
        "old generation acknowledged by current epoch"
    );
    assert_eq!(item(&fixture).attempts, 2);
}

#[test]
fn n37_cancel_leaves_independent_source_dispatch_live() {
    use super::n37_support::{item, scope};
    use maestro_kernel::acquisition::{DispatchRequest, Error, LeaseRequest};
    use std::time::Duration;
    let fixture = Fixture::new();
    let lease = LeaseRequest {
        holder: "foreign",
        now: fixture.context.now,
        term: Duration::from_secs(30),
    };
    let foreign = fixture
        .db
        .lease_source("independent", &scope(), lease)
        .unwrap();
    let row = fixture
        .db
        .enqueue(&foreign, &item(&fixture).request, fixture.context.now)
        .unwrap();
    fixture
        .db
        .lease(
            &foreign,
            row.id,
            DispatchRequest {
                lease,
                max_attempts: 3,
            },
        )
        .unwrap();
    fixture
        .db
        .release_source(&fixture.context.writer, fixture.context.now)
        .unwrap();
    assert!(
        matches!(
            fixture.db.lease(
                &foreign,
                row.id,
                DispatchRequest {
                    lease,
                    max_attempts: 3
                }
            ),
            Err(Error::Unavailable)
        ),
        "independent live dispatch expired"
    );
    let rows = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "independent",
        None,
        100,
    )
    .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows.first().unwrap().attempts, 1);
    fixture
        .db
        .release_source(&foreign, fixture.context.now)
        .unwrap();
}

#[test]
fn n37_frozen_inputs_compare_every_policy_authority_and_profile_binding() {
    use super::n37_support::scope;
    use maestro_acquisition::lifecycle::resume::same_inputs;
    use serde_json::json;
    let fixture = Fixture::new();
    let original = json!({
        "collection":"collection", "resources":["policy", "profile"],
        "os_principal":"reader", "kernel_principal":"reader",
        "scope":scope().as_str(), "mode":"full", "run_now":1
    });
    let before = fixture
        .db
        .retain(&scope(), &serde_json::to_vec(&original).unwrap(), &[])
        .unwrap();
    let mut continuation = original.clone();
    continuation["run_now"] = json!(2);
    continuation["mode"] = json!("incremental");
    let after = fixture
        .db
        .retain(&scope(), &serde_json::to_vec(&continuation).unwrap(), &[])
        .unwrap();
    assert!(same_inputs(&fixture.db, "reader", before, after).unwrap());
    for key in [
        "collection",
        "resources",
        "os_principal",
        "kernel_principal",
        "scope",
    ] {
        let mut changed = continuation.clone();
        changed[key] = json!("changed");
        let after = fixture
            .db
            .retain(&scope(), &serde_json::to_vec(&changed).unwrap(), &[])
            .unwrap();
        assert!(
            !same_inputs(&fixture.db, "reader", before, after).unwrap(),
            "ignored frozen {key}"
        );
    }
    assert!(same_inputs(&fixture.db, "denied", before, after).is_err());
}

#[test]
fn n37_cancel_cannot_pair_current_writer_with_another_attempt_receipt() {
    use super::n37_support::{item, receipt, scope};
    use maestro_acquisition::capture::{Cancellation, cancel_run};
    use maestro_kernel::acquisition::{
        Handle, InventoryPage, InventorySchema, ItemDisposition, LeaseRequest, Stage, StageItem,
        Status,
    };
    use std::time::Duration;
    let mut fixture = Fixture::new();
    let first = receipt(&mut fixture);
    let mut other = first.clone();
    other.attempt = Handle::new();
    other.run = Handle::new();
    fixture.db.begin(&scope(), &other).unwrap();
    let frontier = vec![item(&fixture)];
    let pages = vec![InventoryPage {
        schema: InventorySchema::V1,
        partition: fixture.envelope.inputs,
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
        receipt: &other,
        pages: &pages,
        frontier: &frontier,
    };
    assert!(cancel_run(&fixture.db, &cancellation).is_err());
    for receipt in [&first, &other] {
        let stored = fixture
            .db
            .inspect("reader", receipt.attempt)
            .unwrap()
            .unwrap();
        assert_eq!(stored.status, Status::Pending);
        assert!(stored.inventories.is_empty());
    }
    assert!(
        fixture
            .db
            .lease_source(
                "notes",
                &scope(),
                LeaseRequest {
                    holder: "contender",
                    now: fixture.context.now,
                    term: Duration::from_secs(30)
                }
            )
            .is_err(),
        "mismatched cancellation released ownership"
    );
    assert_eq!(item(&fixture).attempts, 2);
}

#[test]
fn n37_attempt_ceiling_survives_source_takeover_without_reset() {
    use super::n37_support::{item, redispatch, scope};
    use maestro_kernel::acquisition::{DispatchRequest, LeaseRequest};
    use std::time::Duration;
    let mut fixture = Fixture::new();
    fixture.prepare().unwrap();
    redispatch(&mut fixture);
    fixture.context.now += Duration::from_secs(31);
    let lease = LeaseRequest {
        holder: "third",
        now: fixture.context.now,
        term: Duration::from_secs(30),
    };
    let writer = fixture.db.lease_source("notes", &scope(), lease).unwrap();
    assert!(
        fixture
            .db
            .lease(
                &writer,
                fixture.context.item.item,
                DispatchRequest {
                    lease,
                    max_attempts: 2
                }
            )
            .is_err(),
        "takeover reset durable attempt budget"
    );
    assert_eq!(item(&fixture).attempts, 2);
    let third = fixture
        .db
        .lease(
            &writer,
            fixture.context.item.item,
            DispatchRequest {
                lease,
                max_attempts: 3,
            },
        )
        .unwrap();
    assert!(third.epoch > fixture.context.item.epoch);
    assert_eq!(item(&fixture).attempts, 3);
}
