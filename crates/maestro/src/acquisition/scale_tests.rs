//! Executed SQL and actual frontier scan counters for N36's scale contract.
use super::{
    flow_edges::Fixture,
    flow_site::Revision,
    flow_tests::Grants,
    history_tests::{cover_history, historical},
    sync_discovery::{SCAN_ITEMS, SCANS},
};
use maestro_acquisition::lifecycle::full::Mode;
use maestro_kernel::acquisition::{Enumeration, Partitions, Status};
use serde_json::json;
use std::{
    cell::Cell,
    time::{Duration, UNIX_EPOCH},
};

/// Measure only the final full sweep; prior runs supply real durable history.
fn sweep(items: usize, history: u64) -> (u64, u64, usize, usize) {
    let mut fixture = Fixture::new(|policy| {
        policy["sources"][0]["seeds"] = json!(
            (0..items)
                .map(|index| { format!("https://garden.example/docs/legacy-{index}") })
                .collect::<Vec<_>>()
        );
    });
    fixture.site.revision = Revision::ScaleHtml;
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    for index in 0..history {
        let report = fixture
            .run_window(
                999,
                Mode::Full,
                now + Duration::from_secs(index),
                &Grants::default(),
            )
            .unwrap();
        assert_eq!(report.status, Status::Complete, "{report:?}");
    }
    SCANS.set(0);
    SCAN_ITEMS.set(0);
    let statements = fixture.db.statement_count();
    let lookups = fixture.db.item_lookup_count();
    let report = fixture
        .run_window(
            999,
            Mode::Full,
            now + Duration::from_secs(history),
            &Grants::default(),
        )
        .unwrap();
    assert_eq!(report.status, Status::Complete, "{report:?}");
    let counts = (
        fixture.db.statement_count() - statements,
        fixture.db.item_lookup_count() - lookups,
        SCANS.with(Cell::get),
        SCAN_ITEMS.with(Cell::get),
    );
    println!(
        concat!(
            "items={}, history={}, statements={}, item lookups={}, ",
            "frontier scans={}, scanned items={}"
        ),
        items, history, counts.0, counts.1, counts.2, counts.3
    );
    fixture.finish();
    counts
}

#[test]
fn n36_scale_full_sweep_queries_and_scans_are_linear() {
    let n = sweep(4, 1);
    let twice = sweep(8, 1);
    assert!(twice.0 <= 2 * n.0 + 40, "SQL grew faster than items");
    assert_eq!(
        (n.2, twice.2),
        (1, 1),
        "must traverse captured children once"
    );
    assert_eq!((n.3, twice.3), (4, 8), "quadratic frontier scans");
}

#[test]
fn n36_scale_recovery_never_revisits_historical_items() {
    let one = sweep(8, 1);
    let five = sweep(8, 5);
    assert!(
        five.0 <= one.0 + 40,
        "historical item SQL grew: {one:?} -> {five:?}"
    );
    assert_eq!(five.1, one.1, "historical item capture lookups grew");
    assert_eq!((five.2, five.3), (1, 8));
}

#[test]
fn n36_scale_thousand_item_reuse_has_no_item_lookup_sql_and_measures_retention() {
    let fixture = Fixture::new(|policy| {
        policy["sources"][0]["seeds"] = json!(["https://garden.example/docs/legacy-0"]);
        policy["sources"][0]["discovery"] = json!([]);
    });
    historical(&fixture, 1000, true, "reader");
    cover_history(&fixture);
    let connection = rusqlite::Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    let bytes = || {
        connection
            .query_row("SELECT sum(bytes) FROM artifacts", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap()
    };
    let before_bytes = bytes();
    let before = fixture.db.item_lookup_count();
    let report = fixture
        .run_window(
            999,
            Mode::Incremental,
            UNIX_EPOCH + Duration::from_secs(1_000_001),
            &Grants::default(),
        )
        .unwrap();
    assert_eq!(report.status, Status::Complete, "{report:?}");
    assert_eq!(report.completed.len(), 1000);
    assert_eq!(
        fixture.db.item_lookup_count() - before,
        0,
        "reuse did per-item lookups"
    );
    assert!(fixture.site.requests.lock().unwrap().is_empty());
    let retained = bytes() - before_bytes;
    let chunks = fixture
        .db
        .summary_page(&fixture.scope, "notes", None, 100)
        .unwrap();
    let verification_bytes: usize = chunks
        .iter()
        .filter(|summary| {
            summary.run == report.run.unwrap() && summary.kind == Enumeration::Verification
        })
        .map(|summary| {
            let state = fixture
                .db
                .partition(&fixture.scope, summary.id)
                .unwrap()
                .unwrap();
            serde_json::to_vec(state.batches.first().unwrap())
                .unwrap()
                .len()
        })
        .sum();
    println!(
        concat!(
            "items=1000, incremental item lookups=0, verification artifact bytes={}, ",
            "all new artifact bytes={}"
        ),
        verification_bytes, retained
    );
    assert!(verification_bytes > 0 && retained > i64::try_from(verification_bytes).unwrap());
    drop(connection);
    fixture.finish();
}
