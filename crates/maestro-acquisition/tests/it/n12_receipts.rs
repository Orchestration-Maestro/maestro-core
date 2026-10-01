//! Reconciled inventories finalize N06's unique immutable run receipts.
use super::n12_support::Fixture;
use maestro_acquisition::capture::{Captures, finish_run, reconcile};
use maestro_kernel::{
    acquisition::{
        BudgetUsage, Frontier, Handle, InventoryPage, InventorySchema, ItemDisposition, Reason,
        Receipt, ReceiptError, ReceiptSchema, Receipts, Stage, StageItem, Status,
    },
    artifact::Digest,
    scope::Scope,
};

use std::{fs, slice};

#[test]
fn n12_reconciled_run_retains_frozen_inputs_and_non_success() {
    for disposition in [
        ItemDisposition::Accepted,
        ItemDisposition::Blocked,
        ItemDisposition::Refused,
        ItemDisposition::Pending,
    ] {
        let fixture = Fixture::new();
        let handle = fixture.prepare().unwrap();
        if disposition == ItemDisposition::Accepted {
            fixture
                .db
                .acknowledge_capture(&fixture.context, handle)
                .unwrap();
        }
        let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
        let partition = fixture
            .db
            .retain(&scope, b"complete partition", &[])
            .unwrap();
        let page = InventoryPage {
            schema: InventorySchema::V1,
            partition,
            stage: Stage::Capture,
            complete: disposition != ItemDisposition::Pending,
            items: vec![StageItem {
                item: fixture.envelope.item,
                disposition,
                evidence: Some(handle),
            }],
        };
        let mut receipt = Receipt {
            schema: ReceiptSchema::V1,
            run: fixture.envelope.run,
            attempt: Handle::new(),
            inputs: fixture.envelope.inputs,
            tightening: vec![],
            inventories: vec![],
            attempts: 0,
            budget: BudgetUsage {
                elapsed_ms: 0,
                response_bytes: 4,
                staging_bytes: 4,
            },
            downstream: vec![],
            status: Status::Pending,
            reason: Reason::None,
        };
        fixture.db.begin(&scope, &receipt).unwrap();
        receipt.reason = if disposition == ItemDisposition::Refused {
            Reason::Transport
        } else {
            Reason::None
        };
        let frontier = Frontier::page(
            &fixture.db,
            &fixture.db.visible("reader").unwrap(),
            "notes",
            None,
            10,
        )
        .unwrap();
        let outcome = finish_run(
            &fixture.db,
            &scope,
            &receipt,
            slice::from_ref(&page),
            &frontier,
        )
        .unwrap();
        assert_eq!(outcome.success(), disposition == ItemDisposition::Accepted);
        let persisted = fixture
            .db
            .inspect("reader", receipt.attempt)
            .unwrap()
            .unwrap();
        assert_eq!(persisted.status, outcome.status);
        assert_eq!(persisted.attempts, 1);
        assert_eq!(persisted.inputs, receipt.inputs);
        assert_eq!(persisted.inventories.len(), 1);
        assert!(finish_run(&fixture.db, &scope, &receipt, &[page], &frontier).is_err());
    }
}
#[test]
fn n12_missing_disposition_is_incomplete_not_accepted() {
    let fixture = Fixture::new();
    let frontier = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        10,
    )
    .unwrap();
    let outcome = reconcile(
        &[],
        &frontier,
        Reason::None,
        (
            &fixture.db,
            &"workspace/default/collection/garden".parse().unwrap(),
        ),
    )
    .unwrap();
    assert_eq!(outcome.status, Status::Partial);
    assert_eq!(outcome.attempted_items, 1);
    assert_eq!(outcome.distinct_items, 1);
    assert_eq!(
        outcome.pending_without_disposition,
        vec![fixture.envelope.item]
    );
    assert_eq!(outcome.stages[0].pending, 1);
}

#[test]
fn n12_missing_disposition_refuses_finish_and_keeps_pending_receipt() {
    let fixture = Fixture::new();
    let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
    let receipt = Receipt {
        schema: ReceiptSchema::V1,
        run: fixture.envelope.run,
        attempt: Handle::new(),
        inputs: fixture.envelope.inputs,
        tightening: vec![],
        inventories: vec![],
        attempts: 0,
        budget: BudgetUsage {
            elapsed_ms: 0,
            response_bytes: 0,
            staging_bytes: 0,
        },
        downstream: vec![],
        status: Status::Pending,
        reason: Reason::None,
    };
    fixture.db.begin(&scope, &receipt).unwrap();
    let frontier = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        10,
    )
    .unwrap();
    let recorded = fixture.db.check_artifacts().unwrap().recorded;
    assert_eq!(
        finish_run(&fixture.db, &scope, &receipt, &[], &frontier),
        Err(ReceiptError::Invalid)
    );
    assert_eq!(
        fixture
            .db
            .inspect("reader", receipt.attempt)
            .unwrap()
            .unwrap()
            .status,
        Status::Pending
    );
    assert_eq!(fixture.db.check_artifacts().unwrap().recorded, recorded);
}

#[test]
fn n12_declared_cancellation_or_failure_never_becomes_success() {
    for status in [
        Status::Partial,
        Status::Blocked,
        Status::Failed,
        Status::Cancelled,
    ] {
        let fixture = Fixture::new();
        let handle = fixture.prepare().unwrap();
        fixture
            .db
            .acknowledge_capture(&fixture.context, handle)
            .unwrap();
        let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
        let partition = fixture
            .db
            .retain(&scope, b"complete partition", &[])
            .unwrap();
        let page = InventoryPage {
            schema: InventorySchema::V1,
            partition,
            stage: Stage::Capture,
            complete: true,
            items: vec![StageItem {
                item: fixture.envelope.item,
                disposition: ItemDisposition::Accepted,
                evidence: Some(handle),
            }],
        };
        let mut receipt = Receipt {
            schema: ReceiptSchema::V1,
            run: fixture.envelope.run,
            attempt: Handle::new(),
            inputs: fixture.envelope.inputs,
            tightening: vec![],
            inventories: vec![],
            attempts: 0,
            budget: BudgetUsage {
                elapsed_ms: 0,
                response_bytes: 4,
                staging_bytes: 4,
            },
            downstream: vec![],
            status: Status::Pending,
            reason: Reason::None,
        };
        fixture.db.begin(&scope, &receipt).unwrap();
        receipt.status = status;
        let frontier = Frontier::page(
            &fixture.db,
            &fixture.db.visible("reader").unwrap(),
            "notes",
            None,
            10,
        )
        .unwrap();
        let outcome = finish_run(&fixture.db, &scope, &receipt, &[page], &frontier).unwrap();
        assert_eq!(outcome.status, status);
        assert!(!outcome.success());
        assert_eq!(
            fixture
                .db
                .inspect("reader", receipt.attempt)
                .unwrap()
                .unwrap()
                .status,
            status
        );
    }
}

#[test]
fn n12_accepted_evidence_refuses_frozen_inputs() {
    accepted_evidence(0);
    accepted_evidence(1);
}
#[test]
fn n12_accepted_evidence_refuses_raw_body_acknowledgment() {
    accepted_evidence(0);
    accepted_evidence(2);
}
#[test]
fn n12_accepted_evidence_refuses_wrong_scope_item_or_corrupt_payloads() {
    accepted_evidence(0);
    for case in 3..7 {
        accepted_evidence(case);
    }
}
/// The successful neighbour and each independent substitution use real storage.
fn accepted_evidence(case: u8) {
    let fixture = Fixture::new();
    let handle = fixture.prepare().unwrap();
    if case == 2 {
        Frontier::acknowledge(
            &fixture.db,
            &fixture.context.writer,
            &fixture.context.item,
            &fixture.envelope.artifact,
            fixture.context.now,
        )
        .unwrap();
    } else {
        fixture
            .db
            .acknowledge_capture(&fixture.context, handle)
            .unwrap();
    }
    let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
    let partition = fixture
        .db
        .retain(&scope, b"complete partition", &[])
        .unwrap();
    let page = InventoryPage {
        schema: InventorySchema::V1,
        partition,
        stage: Stage::Capture,
        complete: true,
        items: vec![StageItem {
            item: fixture.envelope.item,
            disposition: ItemDisposition::Accepted,
            evidence: Some(if case == 1 {
                fixture.envelope.inputs
            } else {
                handle
            }),
        }],
    };
    let receipt = pending_receipt(&fixture);
    fixture.db.begin(&scope, &receipt).unwrap();
    let frontier = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        10,
    )
    .unwrap();
    let mut scope = scope;
    let mut page = page;
    let mut frontier = frontier;
    match case {
        3 => scope = "workspace/default/collection/other".parse().unwrap(),
        4 => {
            let other = Handle::new();
            page.items.first_mut().unwrap().item = other;
            frontier.first_mut().unwrap().id = other.to_string().parse().unwrap();
        }
        5 | 6 => {
            let digest = if case == 5 {
                fixture.envelope.artifact.clone()
            } else {
                Digest::of(&serde_json::to_vec(&fixture.envelope).unwrap())
            };
            let hex = digest.as_str();
            fs::write(
                fixture
                    .root
                    .join("artifacts/sha256")
                    .join(&hex[..2])
                    .join(&hex[2..4])
                    .join(hex),
                b"corrupt",
            )
            .unwrap();
        }
        _ => {}
    }
    assert_eq!(
        reconcile(
            slice::from_ref(&page),
            &frontier,
            Reason::None,
            (&fixture.db, &scope)
        )
        .is_ok(),
        case == 0
    );
    let result = finish_run(
        &fixture.db,
        &scope,
        &receipt,
        slice::from_ref(&page),
        &frontier,
    );
    assert_eq!(result.is_ok(), case == 0, "case {case}");
}

/// Synthetic frozen pending run for capture evidence substitution tests.
fn pending_receipt(fixture: &Fixture) -> Receipt {
    Receipt {
        schema: ReceiptSchema::V1,
        run: fixture.envelope.run,
        attempt: Handle::new(),
        inputs: fixture.envelope.inputs,
        tightening: vec![],
        inventories: vec![],
        attempts: 0,
        budget: BudgetUsage {
            elapsed_ms: 0,
            response_bytes: 4,
            staging_bytes: 4,
        },
        downstream: vec![],
        status: Status::Pending,
        reason: Reason::None,
    }
}
