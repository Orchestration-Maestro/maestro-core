//! N06 review regressions for terminal reconciliation, bounds and privacy.
use super::{
    n04_frontier_support::Scratch,
    n06_store_scoped_receipts_and_content_free_progress_events::{
        grant, inventory, receipt, scope,
    },
};
use maestro_kernel::{
    acquisition::{
        Handle, ItemDisposition, Progress, Reason, ReceiptError, Receipts, Stage, StageItem, Status,
    },
    journal::Filter,
    scope::Right,
    store::Database,
};
use serde_json::Value;
use std::{collections::BTreeSet, iter::once, sync::Barrier, thread};

#[test]
fn n06_review_dropped_pending_inventory() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut run = receipt(&db);
    let page = inventory(run.inputs, ItemDisposition::Pending);
    let handle = db.retain_inventory(&scope("docs"), &page).unwrap();
    run.inventories = vec![handle];
    db.begin(&scope("docs"), &run).unwrap();
    assert_eq!(db.validate_finish(&run), Err(ReceiptError::Invalid));
    run.status = Status::Partial;
    run.reason = Reason::Transport;
    db.validate_finish(&run).unwrap();
    assert_eq!(
        db.inspect("reader", run.attempt).unwrap().unwrap().status,
        Status::Pending
    );
    run.inventories.clear();
    run.status = Status::Complete;
    run.reason = Reason::None;
    assert_eq!(db.validate_finish(&run), Err(ReceiptError::Invalid));
    assert!(db.finish(&run).is_err());
}

#[test]
fn n06_review_finish_at_reference_bound() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut run = receipt(&db);
    run.tightening = vec![run.inputs; 999];
    assert_eq!(1 + run.tightening.len(), 1000);
    assert!(db.begin(&scope("docs"), &run).is_ok());
    run.status = Status::Failed;
    run.reason = Reason::Transport;
    assert!(db.finish(&run).is_ok());
}

#[test]
fn n06_review_finish_at_reference_bound_distinct() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut run = receipt(&db);
    run.tightening = (0..999)
        .map(|i| {
            db.retain(&scope("docs"), i.to_string().as_bytes(), &[])
                .unwrap()
        })
        .collect();
    let references: BTreeSet<_> = once(run.inputs)
        .chain(run.tightening.iter().copied())
        .collect();
    assert_eq!(references.len(), 1000);
    assert!(db.begin(&scope("docs"), &run).is_ok());
    run.status = Status::Failed;
    run.reason = Reason::Transport;
    assert!(db.finish(&run).is_ok());
}

#[test]
fn n06_review_full_inventory_with_evidence() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let partition = db
        .retain(&scope("other"), b"synthetic partition", &[])
        .unwrap();
    let mut page = inventory(partition, ItemDisposition::Accepted);
    page.items = (0..1000)
        .map(|i| StageItem {
            item: Handle::new(),
            disposition: ItemDisposition::Accepted,
            evidence: Some(
                db.retain(&scope("docs"), i.to_string().as_bytes(), &[])
                    .unwrap(),
            ),
        })
        .collect();
    let evidence: BTreeSet<_> = page
        .items
        .iter()
        .map(|item| item.evidence.unwrap())
        .collect();
    assert_eq!(evidence.len(), 1000);
    assert!(!evidence.contains(&partition));
    assert_eq!(page.stage, Stage::Capture);
    let handle = db.retain_inventory(&scope("docs"), &page).unwrap();
    assert!(db.read("reader", handle).unwrap().is_none());
    grant(&db, "other");
    assert!(db.read("reader", handle).unwrap().is_some());
    db.revoke("reader", &scope("other"), Right::Read, "owner")
        .unwrap();
    assert!(db.read("reader", handle).unwrap().is_none());
}

#[test]
fn n06_review_concurrent_finish() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let run = receipt(&db);
    db.begin(&scope("docs"), &run).unwrap();
    let barrier = Barrier::new(2);
    let outcomes = thread::scope(|threads| {
        let handles: Vec<_> = [
            (Status::Failed, Reason::Transport),
            (Status::Blocked, Reason::Expired),
        ]
        .into_iter()
        .map(|(status, reason)| {
            let mut terminal = run.clone();
            terminal.status = status;
            terminal.reason = reason;
            let db = &db;
            let barrier = &barrier;
            threads.spawn(move || {
                barrier.wait();
                let result = db.finish(&terminal);
                (terminal, result)
            })
        })
        .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(
        outcomes.iter().filter(|(_, result)| result.is_ok()).count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|(_, result)| *result == Err(ReceiptError::Conflict))
            .count(),
        1
    );
    let winner = &outcomes
        .iter()
        .find(|(_, result)| result.is_ok())
        .unwrap()
        .0;
    assert_eq!(
        db.inspect("reader", run.attempt).unwrap().as_ref(),
        Some(winner)
    );
    let stream = format!("acquisition/{}", run.attempt);
    let filter = Filter {
        stream: &stream,
        after: 0,
        r#type: None,
    };
    let events = db.events(&db.visible("reader").unwrap(), &filter).unwrap();
    assert_eq!(events.len(), 2);
    let progress: Vec<_> = events
        .iter()
        .map(|event| Progress::parse(&serde_json::to_vec(&event.data).unwrap()).unwrap())
        .collect();
    assert_eq!(progress[0].status, Status::Pending);
    assert_eq!(progress[1].status, winner.status);
    assert_eq!(progress[1].reason, winner.reason);
    drop(db);
    let db = Database::open_in(&root).unwrap();
    assert_eq!(
        db.inspect("reader", run.attempt).unwrap().as_ref(),
        Some(winner)
    );
    let reopened = db.events(&db.visible("reader").unwrap(), &filter).unwrap();
    assert_eq!(reopened, events);
}

#[test]
fn n06_review_missing_evidence_field() {
    let item = StageItem {
        item: Handle::new(),
        disposition: ItemDisposition::Pending,
        evidence: None,
    };
    let mut encoded = serde_json::to_value(&item).unwrap();
    assert_eq!(encoded["evidence"], Value::Null);
    assert_eq!(
        encoded.as_object_mut().unwrap().remove("evidence"),
        Some(Value::Null)
    );
    assert!(serde_json::from_value::<StageItem>(encoded).is_err());
}

#[test]
fn n06_terminal_inventory_can_replace_and_repack_pages() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut run = receipt(&db);
    let mut first = inventory(run.inputs, ItemDisposition::Pending);
    let mut second = inventory(run.inputs, ItemDisposition::Pending);
    run.inventories = [&first, &second]
        .map(|page| db.retain_inventory(&scope("docs"), page).unwrap())
        .to_vec();
    db.begin(&scope("docs"), &run).unwrap();
    first.items[0].disposition = ItemDisposition::Accepted;
    second.items[0].disposition = ItemDisposition::Denied;
    first.items.extend(second.items);
    run.inventories = vec![db.retain_inventory(&scope("docs"), &first).unwrap()];
    run.status = Status::Complete;
    db.finish(&run).unwrap();
    assert_eq!(db.inspect("reader", run.attempt).unwrap(), Some(run));
}

#[test]
fn n06_terminal_inventory_cannot_substitute_item_or_stage() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let mut run = receipt(&db);
    let mut page = inventory(run.inputs, ItemDisposition::Pending);
    run.inventories = vec![db.retain_inventory(&scope("docs"), &page).unwrap()];
    db.begin(&scope("docs"), &run).unwrap();
    page.items[0].disposition = ItemDisposition::Accepted;
    let item = page.items[0].item;
    page.items[0].item = Handle::new();
    run.status = Status::Complete;
    run.inventories = vec![db.retain_inventory(&scope("docs"), &page).unwrap()];
    assert_eq!(db.finish(&run), Err(ReceiptError::Invalid));
    page.items[0].item = item;
    page.stage = Stage::Extraction;
    run.inventories = vec![db.retain_inventory(&scope("docs"), &page).unwrap()];
    assert_eq!(db.finish(&run), Err(ReceiptError::Invalid));
    run.status = Status::Partial;
    run.reason = Reason::Transport;
    assert_eq!(db.finish(&run), Err(ReceiptError::Invalid));
}

#[test]
fn n06_explicit_null_evidence_is_accepted() {
    let item = StageItem {
        item: Handle::new(),
        disposition: ItemDisposition::Pending,
        evidence: None,
    };
    let encoded = serde_json::to_value(&item).unwrap();
    assert_eq!(encoded["evidence"], Value::Null);
    assert_eq!(serde_json::from_value::<StageItem>(encoded).unwrap(), item);
}

#[test]
fn n06_terminal_snapshot_keeps_historical_scopes() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut run = receipt(&db);
    let restricted = db
        .retain(&scope("other"), b"synthetic tightening", &[])
        .unwrap();
    run.tightening.push(restricted);
    db.begin(&scope("docs"), &run).unwrap();
    run.tightening.clear();
    run.status = Status::Complete;
    db.finish(&run).unwrap();
    assert!(db.inspect("reader", run.attempt).unwrap().is_none());
    assert!(db.page("reader", run.run, None, 1).unwrap().is_empty());
    grant(&db, "other");
    assert_eq!(
        db.inspect("reader", run.attempt).unwrap(),
        Some(run.clone())
    );
    assert_eq!(
        db.page("reader", run.run, None, 1).unwrap()[0].receipt,
        run.attempt
    );
    db.revoke("reader", &scope("other"), Right::Read, "owner")
        .unwrap();
    assert!(db.inspect("reader", run.attempt).unwrap().is_none());
    assert!(db.page("reader", run.run, None, 1).unwrap().is_empty());
}

#[test]
fn n06_page_applies_visibility_before_limit() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut denied = receipt(&db);
    let restricted = db
        .retain(&scope("other"), b"synthetic tightening", &[])
        .unwrap();
    denied.tightening.push(restricted);
    let mut allowed = denied.clone();
    // Fixed opaque IDs guarantee order without a clock or random-ID assumption.
    denied.attempt = "00000000000000000000000001".parse().unwrap();
    allowed.attempt = "00000000000000000000000002".parse().unwrap();
    allowed.tightening.clear();
    db.begin(&scope("docs"), &denied).unwrap();
    db.begin(&scope("docs"), &allowed).unwrap();
    assert!(db.inspect("reader", denied.attempt).unwrap().is_none());
    let page = db.page("reader", allowed.run, None, 1).unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].receipt, allowed.attempt);
    assert!(
        db.page("reader", allowed.run, Some(allowed.attempt), 1)
            .unwrap()
            .is_empty()
    );
}
