//! Deterministic reader costs and the opt-in N14 large-history profile.
use super::{
    flow_edges::{Fixture, clean},
    history_tests::historical,
    inspect::inspect,
};
use maestro_kernel::acquisition::Status;
use std::time::Instant;

#[test]
fn s6_sync_reader_opens_do_not_grow_with_acknowledged_history() {
    let counts: Vec<_> = [10, 40]
        .into_iter()
        .map(|rows| {
            let fixture = Fixture::new(clean);
            historical(&fixture, rows, true, "reader");
            let before = fixture.db.reader_opens();
            let report = fixture.sync();
            let opens = fixture.db.reader_opens() - before;
            assert_eq!(report.status, Status::Complete, "{report:?}");
            assert_eq!(report.completed.len(), rows + 3);
            println!("history={rows}, sync reader opens={opens}");
            fixture.finish();
            opens
        })
        .collect();
    assert_eq!(
        counts.first(),
        counts.last(),
        "reader opens grew with history"
    );
    assert!(counts.into_iter().all(|opens| opens <= 4));
}

#[test]
#[ignore = "opt-in 2,500-row N14 performance measurement"]
fn s6_sync_large_history_profile() {
    let fixture = Fixture::new(clean);
    let start = Instant::now();
    historical(&fixture, 2500, true, "reader");
    println!("historical generation {:?}", start.elapsed());
    let before = fixture.db.reader_opens();
    let start = Instant::now();
    let report = fixture.try_sync_page_size(1000).unwrap();
    println!("historical sync {:?}", start.elapsed());
    println!("sync reader opens {}", fixture.db.reader_opens() - before);
    assert_eq!(report.status, Status::Complete, "{report:?}");
    assert_eq!(report.completed.len(), 2503);
    let start = Instant::now();
    let inspected = inspect(&fixture.db, "reader", report.receipt.unwrap()).unwrap();
    println!("historical inspect {:?}", start.elapsed());
    assert_eq!(inspected.completed.len(), 2503);
    fixture.finish();
}

#[test]
fn s6_capture_and_authorization_helpers_share_one_read_unit() {
    use maestro_kernel::{
        acquisition::{Captures, Frontier, Receipts},
        scope::Scope,
    };
    let fixture = Fixture::new(clean);
    historical(&fixture, 1, true, "reader");
    let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
    let scopes = fixture.db.visible("reader").unwrap();
    let items = Frontier::page(&fixture.db, &scopes, "notes", None, 2).unwrap();
    assert_eq!(fixture.db.reader_opens(), 1);
    let item = items.first().unwrap();
    let handle = fixture.db.capture_for(&scope, item).unwrap().unwrap();
    assert_eq!(
        fixture.db.reader_opens(),
        1,
        "capture lookup opened a nested reader"
    );
    fixture.db.verify_capture(&scope, item, handle).unwrap();
    assert_eq!(
        fixture.db.reader_opens(),
        1,
        "capture verification opened a nested reader"
    );
    assert!(fixture.db.read("reader", handle).unwrap().is_some());
    assert_eq!(
        fixture.db.reader_opens(),
        1,
        "authorization opened a nested reader"
    );
    fixture.finish();
}

#[test]
fn s6_receipt_inventory_validation_shares_one_read_unit() {
    use super::{command::new_receipt, output::Report};
    use maestro_kernel::{
        acquisition::{Handle, InventoryPage, InventorySchema, Reason, Receipts, Stage},
        scope::Scope,
    };
    let fixture = Fixture::new(clean);
    let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
    let inputs = fixture.db.retain(&scope, b"synthetic inputs", &[]).unwrap();
    let mut report = Report::new();
    report.run = Some(Handle::new());
    report.receipt = Some(Handle::new());
    let mut receipt = new_receipt(&report, inputs).unwrap();
    fixture.db.begin(&scope, &receipt).unwrap();
    for _ in 0..4 {
        receipt.inventories.push(
            fixture
                .db
                .retain_inventory(
                    &scope,
                    &InventoryPage {
                        schema: InventorySchema::V1,
                        partition: inputs,
                        stage: Stage::Capture,
                        complete: true,
                        items: vec![],
                    },
                )
                .unwrap(),
        );
    }
    receipt.status = Status::Complete;
    receipt.reason = Reason::None;
    let before = fixture.db.reader_opens();
    fixture.db.finish(&receipt).unwrap();
    assert_eq!(
        fixture.db.reader_opens(),
        before,
        "inventory validation opened a nested reader"
    );
    fixture.finish();
}
