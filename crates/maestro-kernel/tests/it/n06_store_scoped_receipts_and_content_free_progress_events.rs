//! N06: opaque receipt views, durable attempts, and content-free event sinks.
#![cfg(test)]
use super::n04_frontier_support::Scratch;
use maestro_kernel::{
    acquisition::{
        BudgetUsage, Handle, InventoryPage, InventorySchema, ItemDisposition, Progress, Reason,
        Receipt, ReceiptSchema, Receipts, Stage, StageItem, Status,
    },
    journal::{Envelope, Filter, Machine},
    scope::{Right, Scope},
    store::Database,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::fmt::Write as _;

/// Synthetic collection boundary.
fn scope(name: &str) -> Scope {
    format!("workspace/default/collection/{name}")
        .parse()
        .unwrap()
}
/// Gives the test reader one exact scope.
fn grant(db: &Database, name: &str) {
    db.grant("reader", &scope(name), Right::Read, "owner")
        .unwrap();
}
/// Frozen inputs contain canaries, never event attributes.
fn receipt(store: &dyn Receipts) -> Receipt {
    let inputs = store
        .retain(
            &scope("docs"),
            b"https://private.test/?token=URL_CANARY#secret SAMPLE_CANARY \
            REPORT_CANARY ERROR_CANARY",
            &[],
        )
        .unwrap();
    Receipt {
        schema: ReceiptSchema::V1,
        run: Handle::new(),
        attempt: Handle::new(),
        inputs,
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
    }
}
/// One page of distinct capture items; dispatch counts stay in the receipt.
fn inventory(partition: Handle, disposition: ItemDisposition) -> InventoryPage {
    InventoryPage {
        schema: InventorySchema::V1,
        partition,
        stage: Stage::Capture,
        complete: true,
        items: vec![StageItem {
            item: Handle::new(),
            disposition,
            evidence: None,
        }],
    }
}

#[test]
fn n06_pending_restart_terminal_once_and_separate_attempts() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut first = receipt(&db);
    let encoded = serde_json::to_vec(&first).unwrap();
    assert_eq!(serde_json::from_slice::<Receipt>(&encoded).unwrap(), first);
    db.begin(&scope("docs"), &first).unwrap();
    assert!(db.begin(&scope("docs"), &first).is_err());
    drop(db);
    let db = Database::open_in(&root).unwrap();
    assert_eq!(
        db.inspect("reader", first.attempt).unwrap().unwrap().status,
        Status::Pending
    );
    let original_inputs = first.inputs;
    first.status = Status::Blocked;
    first.reason = Reason::Expired;
    db.finish(&first).unwrap();
    assert!(db.finish(&first).is_err());
    let saved = db.inspect("reader", first.attempt).unwrap().unwrap();
    assert_eq!(saved.inputs, original_inputs);
    assert_eq!(saved.status, Status::Blocked);
    assert_eq!(saved.reason, Reason::Expired);
    let mut next = first.clone();
    next.attempt = Handle::new();
    next.status = Status::Pending;
    next.reason = Reason::None;
    db.begin(&scope("docs"), &next).unwrap();
    next.status = Status::Complete;
    db.finish(&next).unwrap();
    let page = db.page("reader", first.run, None, 1).unwrap();
    assert_eq!(page.len(), 1);
    let second = db
        .page("reader", first.run, Some(page[0].receipt), 1)
        .unwrap();
    assert_eq!(second.len(), 1);
    assert_ne!(page[0].receipt, second[0].receipt);
    assert!(
        db.page("reader", first.run, Some(second[0].receipt), 1)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        db.inspect("reader", first.attempt).unwrap().unwrap().status,
        Status::Blocked
    );
    for bad in [0, 1001] {
        assert!(db.page("reader", first.run, None, bad).is_err());
    }
}

#[test]
fn n06_cross_scope_unknown_and_transitive_revocation_reveal_nothing() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let secret = db.retain(&scope("other"), b"REPORT_CANARY", &[]).unwrap();
    let middle = db
        .retain(&scope("docs"), b"SAMPLE_CANARY", &[secret])
        .unwrap();
    let mut run = receipt(&db);
    run.tightening.push(middle);
    db.begin(&scope("docs"), &run).unwrap();
    assert!(db.read("reader", secret).unwrap().is_none());
    assert!(db.read("reader", middle).unwrap().is_none());
    assert!(db.inspect("reader", run.attempt).unwrap().is_none());
    assert!(db.inspect("reader", Handle::new()).unwrap().is_none());
    assert!(db.page("reader", run.run, None, 10).unwrap().is_empty());
    grant(&db, "other");
    assert_eq!(
        db.read("reader", secret).unwrap().unwrap().bytes(),
        b"REPORT_CANARY"
    );
    assert!(db.inspect("reader", run.attempt).unwrap().is_some());
    db.revoke("reader", &scope("other"), Right::Read, "owner")
        .unwrap();
    assert!(db.inspect("reader", run.attempt).unwrap().is_none());
    assert!(db.page("reader", run.run, None, 10).unwrap().is_empty());
    assert!(db.inspect("stranger", run.attempt).unwrap().is_none());
    assert!(db.read("reader", Handle::new()).unwrap().is_none());
}

#[test]
fn n06_canaries_never_reach_journal_notifier_log_or_public_export() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut run = receipt(&db);
    db.begin(&scope("docs"), &run).unwrap();
    run.status = Status::Failed;
    run.reason = Reason::Transport;
    db.finish(&run).unwrap();
    let events = db
        .events(
            &db.visible("reader").unwrap(),
            &Filter {
                stream: &format!("acquisition/{}", run.attempt),
                after: 0,
                r#type: None,
            },
        )
        .unwrap();
    assert_eq!(events.len(), 2);
    let mut sinks = String::new();
    for event in &events {
        let progress = Progress::parse(&serde_json::to_vec(&event.data).unwrap()).unwrap();
        assert_eq!(progress.receipt, run.attempt);
        write!(sinks, "{progress:?}{event:?}").unwrap();
        // The real journal envelope is the notifier/public export boundary.
        sinks.push_str(
            &serde_json::to_string(&Envelope::new(
                event.clone(),
                &Machine::parse("fixture").unwrap(),
            ))
            .unwrap(),
        );
    }
    let protected = db.read("reader", run.inputs).unwrap().unwrap();
    write!(sinks, "{protected:?}").unwrap();
    for canary in [
        "URL_CANARY",
        "SAMPLE_CANARY",
        "REPORT_CANARY",
        "ERROR_CANARY",
        "private.test",
    ] {
        assert!(!sinks.contains(canary), "content escaped: {canary}");
    }
    let unknown = json!({
        "receipt": run.attempt, "status":"failed", "reason":"transport", "error":"ERROR_CANARY"
    });
    let result = Progress::parse(&serde_json::to_vec(&unknown).unwrap());
    assert!(result.is_err());
    let refused = result.unwrap_err();
    assert!(!format!("{refused:?} {refused}").contains("ERROR_CANARY"));
    for field in ["url", "sample", "report", "headers", "credentials"] {
        let mut value = json!({"receipt": run.attempt, "status":"failed", "reason":"transport"});
        value[field] = Value::from("URL_CANARY");
        assert!(Progress::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    for bad in ["URL_CANARY", "https://private.test/?token=URL_CANARY"] {
        let value = json!({"receipt":bad, "status":"failed", "reason":"transport"});
        assert!(Progress::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}

#[test]
fn n06_crash_after_artifact_before_event_preserves_pending_and_replays() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut run = receipt(&db);
    db.begin(&scope("docs"), &run).unwrap();
    run.status = Status::Blocked;
    run.reason = Reason::Expired;
    let sql = Connection::open(root.join("kernel.sqlite3")).unwrap();
    sql.execute_batch(
        "CREATE TRIGGER n06_crash BEFORE INSERT ON events
        WHEN NEW.type = 'maestro.acquisition.progress.v1'
        BEGIN SELECT RAISE(ABORT, 'synthetic crash'); END;",
    )
    .unwrap();
    assert!(db.finish(&run).is_err());
    drop(db);
    let db = Database::open_in(&root).unwrap();
    assert_eq!(
        db.inspect("reader", run.attempt).unwrap().unwrap().status,
        Status::Pending
    );
    let before = db
        .events(
            &db.visible("reader").unwrap(),
            &Filter {
                stream: &format!("acquisition/{}", run.attempt),
                after: 0,
                r#type: None,
            },
        )
        .unwrap();
    assert_eq!(before.len(), 1);
    sql.execute_batch("DROP TRIGGER n06_crash;").unwrap();
    db.finish(&run).unwrap();
    db.collect_garbage().unwrap();
    assert_eq!(
        db.inspect("reader", run.attempt).unwrap().unwrap().reason,
        Reason::Expired
    );
    assert!(db.read("reader", run.inputs).unwrap().is_some());
}

#[test]
fn n06_inventory_pages_keep_stage_units_and_refuse_false_completion() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut run = receipt(&db);
    let partition = db.retain(&scope("docs"), b"REPORT_CANARY", &[]).unwrap();
    let mut page = inventory(partition, ItemDisposition::Pending);
    let pending = db.retain_inventory(&scope("docs"), &page).unwrap();
    run.inventories = vec![pending];
    db.begin(&scope("docs"), &run).unwrap();
    run.attempts = 3;
    run.status = Status::Complete;
    assert!(db.finish(&run).is_err());
    for disposition in [
        ItemDisposition::Blocked,
        ItemDisposition::Refused,
        ItemDisposition::Discovered,
    ] {
        page.items[0].disposition = disposition;
        run.inventories = vec![db.retain_inventory(&scope("docs"), &page).unwrap()];
        assert!(db.finish(&run).is_err());
    }
    page.items[0].disposition = ItemDisposition::Accepted;
    page.complete = false;
    run.inventories = vec![db.retain_inventory(&scope("docs"), &page).unwrap()];
    assert!(db.finish(&run).is_err());
    page.complete = true;
    let captured = db.retain_inventory(&scope("docs"), &page).unwrap();
    page.stage = Stage::Extraction;
    let extracted = db.retain_inventory(&scope("docs"), &page).unwrap();
    run.inventories = vec![captured, captured];
    assert!(db.finish(&run).is_err());
    page.stage = Stage::Discovery;
    page.items[0].disposition = ItemDisposition::Discovered;
    let discovered = db.retain_inventory(&scope("docs"), &page).unwrap();
    run.inventories = vec![discovered, captured, extracted];
    run.reason = Reason::Expired;
    assert!(db.finish(&run).is_err());
    run.reason = Reason::None;
    assert!(
        db.finish(&run).is_ok(),
        "complete discovery and capture stages must reconcile"
    );
    let view = db.inspect("reader", run.attempt).unwrap().unwrap();
    assert_eq!(view.attempts, 3);
    assert_eq!(view.inventories.len(), 3);
    let capture: InventoryPage =
        serde_json::from_slice(db.read("reader", captured).unwrap().unwrap().bytes()).unwrap();
    assert_eq!(capture.items.len(), 1);
    assert_eq!(capture.stage, Stage::Capture);
    page.items.push(page.items[0].clone());
    assert!(db.retain_inventory(&scope("docs"), &page).is_err());
    page.items = (0..1001)
        .map(|_| StageItem {
            item: Handle::new(),
            disposition: ItemDisposition::Accepted,
            evidence: None,
        })
        .collect();
    assert!(db.retain_inventory(&scope("docs"), &page).is_err());
    page.items.truncate(1);
    let forged_page = db
        .retain(&scope("docs"), &serde_json::to_vec(&page).unwrap(), &[])
        .unwrap();
    let mut forged = receipt(&db);
    forged.inventories = vec![forged_page];
    assert!(db.begin(&scope("docs"), &forged).is_err());
}

#[test]
fn n06_invalid_payload_bounds_and_references_refuse() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let run = receipt(&db);
    assert!(
        db.retain(&scope("docs"), b"payload", &[Handle::new()])
            .is_err()
    );
    assert!(
        db.retain(&"workspace/default".parse().unwrap(), b"payload", &[])
            .is_err()
    );
    assert!(
        db.retain(&scope("docs"), &vec![0; 4 * 1024 * 1024 + 1], &[])
            .is_err()
    );
    assert!(
        db.retain(&scope("docs"), b"payload", &vec![run.inputs; 1001])
            .is_err()
    );
    assert!(
        db.retain(&scope("docs"), &vec![0; 4 * 1024 * 1024], &[])
            .is_ok()
    );
    assert!(
        db.retain(&scope("docs"), b"payload", &vec![run.inputs; 1000])
            .is_ok()
    );
}

#[test]
fn n06_frozen_input_substitution_and_nonterminal_finish_refuse() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut run = receipt(&db);
    run.status = Status::Failed;
    assert!(db.begin(&scope("docs"), &run).is_err());
    run.status = Status::Pending;
    run.reason = Reason::Expired;
    assert!(db.begin(&scope("docs"), &run).is_err());
    run.reason = Reason::None;
    db.begin(&scope("docs"), &run).unwrap();
    assert!(db.finish(&run).is_err());
    let mut changed = run.clone();
    changed.status = Status::Failed;
    changed.inputs = db.retain(&scope("docs"), b"substituted", &[]).unwrap();
    assert!(db.finish(&changed).is_err());
    changed = run.clone();
    changed.status = Status::Failed;
    changed.run = Handle::new();
    assert!(db.finish(&changed).is_err());
    let mut missing = run.clone();
    missing.attempt = Handle::new();
    missing.status = Status::Failed;
    assert!(db.finish(&missing).is_err());
    assert!(matches!(db.inspect("reader", Handle::new()), Ok(None)));
    assert_eq!(
        db.inspect("reader", run.attempt).unwrap().unwrap().status,
        Status::Pending
    );
}

#[test]
fn n06_noop_terminal_update_cannot_publish_false_progress() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut run = receipt(&db);
    db.begin(&scope("docs"), &run).unwrap();
    run.status = Status::Failed;
    let sql = Connection::open(root.join("kernel.sqlite3")).unwrap();
    sql.execute_batch(
        "CREATE TRIGGER n06_noop BEFORE UPDATE ON acquisition_receipts
        BEGIN SELECT RAISE(IGNORE); END;",
    )
    .unwrap();
    assert!(db.finish(&run).is_err());
    assert_eq!(
        db.inspect("reader", run.attempt).unwrap().unwrap().status,
        Status::Pending
    );
    let events = db
        .events(
            &db.visible("reader").unwrap(),
            &Filter {
                stream: &format!("acquisition/{}", run.attempt),
                after: 0,
                r#type: None,
            },
        )
        .unwrap();
    assert_eq!(events.len(), 1);
    sql.execute_batch("DROP TRIGGER n06_noop;").unwrap();
    db.finish(&run).unwrap();
}

#[test]
fn n06_versioned_schema_and_item_evidence_scopes_are_checked() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    grant(&db, "docs");
    let mut run = receipt(&db);
    let mut encoded = serde_json::to_value(&run).unwrap();
    assert_eq!(encoded["schema"], "maestro-acquisition-receipt/1");
    encoded["schema"] = Value::from("unsupported/2");
    assert!(serde_json::from_value::<Receipt>(encoded).is_err());
    let evidence = db.retain(&scope("other"), b"ERROR_CANARY", &[]).unwrap();
    let mut page = inventory(run.inputs, ItemDisposition::Denied);
    page.items[0].evidence = Some(evidence);
    let mut encoded = serde_json::to_value(&page).unwrap();
    assert_eq!(encoded["schema"], "maestro-acquisition-inventory/1");
    encoded["schema"] = Value::from("unsupported/2");
    assert!(serde_json::from_value::<InventoryPage>(encoded).is_err());
    let handle = db.retain_inventory(&scope("docs"), &page).unwrap();
    run.inventories.push(handle);
    db.begin(&scope("docs"), &run).unwrap();
    assert!(db.read("reader", handle).unwrap().is_none());
    assert!(db.inspect("reader", run.attempt).unwrap().is_none());
    grant(&db, "other");
    assert!(db.read("reader", handle).unwrap().is_some());
    assert!(db.inspect("reader", run.attempt).unwrap().is_some());
}
