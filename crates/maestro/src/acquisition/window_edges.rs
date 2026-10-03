//! N36 pending targets, chunk crashes and non-text source revalidation.
use super::{
    flow_edges::{Fixture, clean},
    flow_tests::Revision,
    inspect::inspect,
};
use maestro_acquisition::{
    Refusal,
    lifecycle::full::Mode,
    policy::authority::{Authority, AuthorityRefusal, Operation, Permit, Target},
};
use maestro_kernel::{
    acquisition::{
        CaptureEnvelope, Captures, Enumeration, Frontier, PartitionState, Partitions, Receipts,
        Status,
    },
    scope::Scope,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use std::{
    env::consts::OS,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Local verification chunks, excluding offline Links inventories.
pub(super) fn windows(fixture: &Fixture) -> Vec<PartitionState> {
    let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
    fixture
        .db
        .partition_page(&scope, "notes", None, 1000)
        .unwrap()
        .into_iter()
        .map(|id| fixture.db.partition(&scope, id).unwrap().unwrap())
        .filter(|state| state.batches.first().unwrap().partition.kind == Enumeration::Verification)
        .collect()
}

#[test]
fn n36_two_capped_runs_commit_original_target_without_refetch() {
    let fixture = Fixture::new(|policy| {
        clean(policy);
        policy["sources"][0]["limits"]["pages"] = json!(2);
    });
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let first = fixture.sync_window(Mode::Incremental, now);
    assert_eq!(first.status, Status::Partial, "{first:?}");
    assert!(
        windows(&fixture)
            .iter()
            .all(|window| window.accepted.is_none())
    );
    let backward = fixture.sync_window(Mode::Incremental, now - Duration::from_secs(1));
    assert_eq!(
        backward.status,
        Status::Blocked,
        "backwards pending clock accepted: {backward:?}"
    );
    let second = fixture.sync_window(Mode::Incremental, now + Duration::from_secs(5));
    assert_eq!(second.status, Status::Complete, "{second:?}");
    assert_ne!(first.receipt, second.receipt);
    for path in ["/docs/start", "/docs/allowed", "/docs/final"] {
        assert_eq!(
            fixture
                .site
                .requests
                .lock()
                .unwrap()
                .iter()
                .filter(|request| request.as_str() == path)
                .count(),
            1,
            "refetched {path}"
        );
    }
    let accepted: Vec<_> = windows(&fixture)
        .into_iter()
        .filter_map(|window| window.accepted)
        .collect();
    assert!(!accepted.is_empty());
    assert!(
        accepted
            .iter()
            .all(|window| window.watermark == 2_000_000_000)
    );
    let next = fixture.sync_window(Mode::Incremental, now + Duration::from_secs(10));
    assert_eq!(next.status, Status::Complete);
    assert!(
        windows(&fixture)
            .iter()
            .filter(|window| window.batches.first().unwrap().partition.run == next.run.unwrap())
            .all(|window| window.accepted.as_ref().unwrap().watermark == 2_000_010_000),
        "completed pending target was reopened"
    );
    fixture.finish();
}

#[test]
fn n36_chunk_commit_crash_does_not_publish_window_and_continues_without_fetch() {
    chunk_crash(4, false);
    chunk_crash(5, false);
}

#[test]
fn n36_first_verification_crash_recovers_original_target_without_fetch() {
    chunk_crash(3, false);
}

#[test]
fn n36_first_verification_crash_does_not_adopt_foreign_policy() {
    chunk_crash(3, true);
}

/// Fail either verification boundary or receipt finalization at a frozen target.
fn chunk_crash(threshold: usize, change_policy: bool) {
    let mut fixture = Fixture::new(clean);
    let connection = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    connection
        .execute_batch(&format!(
            "CREATE TRIGGER fail_second_chunk BEFORE INSERT ON acquisition_partitions \
        WHEN (SELECT count(*) FROM acquisition_partitions) >= {threshold} \
        BEGIN SELECT RAISE(ABORT, 'synthetic chunk crash'); END;"
        ))
        .unwrap();
    if threshold == 5 {
        connection
            .execute_batch(
                "CREATE TRIGGER fail_finish BEFORE UPDATE OF terminal ON acquisition_receipts
             BEGIN SELECT RAISE(ABORT, 'synthetic receipt crash'); END;",
            )
            .unwrap();
    }
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let first = fixture.sync_window(Mode::Full, now);
    assert_eq!(first.status, Status::Failed, "{first:?}");
    assert!(
        inspect(&fixture.db, "reader", first.receipt.unwrap())
            .unwrap()
            .pending
            .iter()
            .any(|entry| entry.reason == "attempt_not_finalized")
    );
    assert_eq!(
        windows(&fixture)
            .iter()
            .filter(|window| window.accepted.is_some())
            .count(),
        threshold - 3,
        "unexpected committed chunks before crash"
    );
    let requests = fixture.site.requests.lock().unwrap().len();
    connection
        .execute_batch("DROP TRIGGER fail_second_chunk; DROP TRIGGER IF EXISTS fail_finish;")
        .unwrap();
    drop(connection);
    if change_policy {
        let (collection, files) = super::flow_tests::fixture_with(|policy| {
            clean(policy);
            policy["sources"][0]["limits"]["requests"] = json!(4);
        });
        fixture.policy = super::command::resolve(
            &files,
            &collection,
            &maestro_acquisition::Principal {
                id: "reader",
                platform: OS,
                scopes: &fixture.scopes,
            },
        )
        .unwrap();
    }
    let second = fixture.sync_window(Mode::Incremental, now + Duration::from_secs(5));
    assert_eq!(second.status, Status::Complete, "{second:?}");
    assert_eq!(
        fixture.site.requests.lock().unwrap().len(),
        requests + if change_policy { 4 } else { 0 },
        "unexpected continuation fetch count"
    );
    assert!(
        windows(&fixture)
            .iter()
            .filter_map(|window| window.accepted.as_ref())
            .all(|window| window.watermark
                == if change_policy {
                    2_000_005_000
                } else {
                    2_000_000_000
                }),
        "incorrect recovery target"
    );
    fixture.finish();
}

#[test]
fn n36_validator_only_change_keeps_bytes_but_revises_capture_and_receipt_mode() {
    let mut fixture = Fixture::new(clean);
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    assert_eq!(
        fixture.sync_window(Mode::Full, now).status,
        Status::Complete
    );
    let scopes = fixture.db.visible("reader").unwrap();
    let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
    let item = Frontier::page(&fixture.db, &scopes, "notes", None, 1000)
        .unwrap()
        .remove(0);
    let original = fixture.db.capture_for(&scope, &item).unwrap().unwrap();
    let before: CaptureEnvelope = serde_json::from_slice(
        fixture
            .db
            .read("reader", original)
            .unwrap()
            .unwrap()
            .bytes(),
    )
    .unwrap();
    fixture.site.revision = Revision::Metadata;
    let result = fixture.sync_window(Mode::Full, now + Duration::from_secs(1));
    assert_eq!(result.status, Status::Complete, "{result:?}");
    assert!(
        result
            .completed
            .iter()
            .all(|entry| entry.reason == "revalidated_changed"),
        "validator-only change was hidden"
    );
    let current = Frontier::page(&fixture.db, &scopes, "notes", None, 1000)
        .unwrap()
        .into_iter()
        .find(|row| row.id == item.id)
        .unwrap();
    let handle = fixture.db.capture_for(&scope, &current).unwrap().unwrap();
    let after: CaptureEnvelope =
        serde_json::from_slice(fixture.db.read("reader", handle).unwrap().unwrap().bytes())
            .unwrap();
    assert_ne!(handle, original);
    assert_eq!(
        before.artifact, after.artifact,
        "fixture accidentally changed body bytes"
    );
    assert_ne!(before.headers, after.headers);
    let receipt = fixture
        .db
        .inspect("reader", result.receipt.unwrap())
        .unwrap()
        .unwrap();
    let inputs: Value = serde_json::from_slice(
        fixture
            .db
            .read("reader", receipt.inputs)
            .unwrap()
            .unwrap()
            .bytes(),
    )
    .unwrap();
    assert_eq!(inputs.get("mode"), Some(&json!("full")));
    fixture.finish();
}

#[test]
fn n36_partial_terminal_receipt_requires_every_same_run_chunk_accepted() {
    use super::{command::new_receipt, output::Report};
    use maestro_kernel::acquisition::{Batch, Handle, LeaseRequest, Partition, Window};
    for incomplete in [false, true] {
        let fixture = Fixture::new(clean);
        let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
        assert_eq!(
            fixture.sync_window(Mode::Full, now).status,
            Status::Complete
        );
        let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
        let inputs = fixture
            .db
            .retain(
                &scope,
                b"synthetic source-complete, other-source-partial receipt",
                &[],
            )
            .unwrap();
        let mut report = Report::new();
        report.run = Some(Handle::new());
        report.receipt = Some(Handle::new());
        let mut receipt = new_receipt(&report, inputs).unwrap();
        fixture.db.begin(&scope, &receipt).unwrap();
        let real_now = SystemTime::now();
        let writer = fixture
            .db
            .lease_source(
                "notes",
                &scope,
                LeaseRequest {
                    holder: "synthetic",
                    now: real_now,
                    term: Duration::from_secs(30),
                },
            )
            .unwrap();
        for index in 0..2 {
            let batch = Batch {
                partition: Partition {
                    id: Handle::new(),
                    run: receipt.run,
                    kind: Enumeration::Verification,
                    window: Window {
                        start: 2_000_000_000,
                        end: 2_000_001_000,
                        overlap: 0,
                        skew: 0,
                    },
                    max_batches: 1,
                    max_items: 1,
                },
                cursor: None,
                next: None,
                terminal: true,
                verification_final: index == 1,
                stable: !(incomplete && index == 1),
                truncated: false,
                expected: Some(0),
                items: vec![],
                extractor: None,
                parent_keys: None,
                not_enqueued: vec![],
                inventory_overflow: 0,
                parent_depth: None,
                capture: None,
            };
            fixture.db.checkpoint(&writer, &batch, real_now).unwrap();
            if batch.stable {
                fixture
                    .db
                    .commit_partition(&writer, batch.partition.id, real_now)
                    .unwrap();
            }
        }
        receipt.status = Status::Partial;
        fixture.db.finish(&receipt).unwrap();
        fixture.db.release_source(&writer, real_now).unwrap();
        let next = fixture.sync_window(Mode::Incremental, now + Duration::from_secs(5));
        assert_eq!(next.status, Status::Complete, "{next:?}");
        let current: Vec<_> = windows(&fixture)
            .into_iter()
            .filter(|window| window.batches.first().unwrap().partition.run == next.run.unwrap())
            .collect();
        let target = if incomplete {
            2_000_001_000
        } else {
            2_000_005_000
        };
        assert!(
            current
                .iter()
                .all(|window| window.accepted.as_ref().unwrap().watermark == target),
            "partial receipt masked an unaccepted source chunk"
        );
        fixture.finish();
    }
}

#[test]
fn n36_seed_hold_outside_frontier_still_blocks_window_commit() {
    let fixture = Fixture::new(|policy| {
        clean(policy);
        policy["sources"][0]["seeds"] = json!([
            "https://garden.example/docs/start",
            "https://garden.example/docs/held"
        ]);
    });
    let report = fixture
        .run_window(
            2,
            Mode::Full,
            UNIX_EPOCH + Duration::from_secs(2_000_000),
            &SeedHold,
        )
        .unwrap();
    assert_eq!(report.status, Status::Partial, "{report:?}");
    assert!(
        report
            .pending
            .iter()
            .any(|entry| entry.reason == "authority")
    );
    assert!(
        windows(&fixture)
            .iter()
            .all(|window| window.accepted.is_none() && !window.batches.first().unwrap().stable),
        "uninventoried seed hold advanced window"
    );
    fixture.finish();
}

#[test]
fn n36_multiple_incomplete_windows_continue_the_oldest_target() {
    let fixture = Fixture::new(|policy| {
        clean(policy);
        policy["sources"][0]["limits"]["pages"] = json!(2);
    });
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    assert_eq!(fixture.sync_window(Mode::Full, now).status, Status::Partial);
    assert_eq!(
        fixture
            .sync_window(Mode::Full, now + Duration::from_secs(5))
            .status,
        Status::Partial
    );
    let completed = fixture.sync_window(Mode::Incremental, now + Duration::from_secs(10));
    assert_eq!(completed.status, Status::Complete, "{completed:?}");
    let current: Vec<_> = windows(&fixture)
        .into_iter()
        .filter(|window| window.batches.first().unwrap().partition.run == completed.run.unwrap())
        .collect();
    assert!(
        current
            .iter()
            .all(|window| window.accepted.as_ref().unwrap().watermark == 2_000_000_000),
        "newer pending target bypassed oldest window"
    );
    fixture.finish();
}

/// The extra selected seed is currently unauthorized, without touching transport.
#[derive(Debug)]
struct SeedHold;
impl Authority for SeedHold {
    fn decide(
        &self,
        _: &str,
        _: Operation,
        target: &Target,
        _: SystemTime,
    ) -> Result<Permit, AuthorityRefusal> {
        if target.resource.ends_with("/held") {
            return Err(AuthorityRefusal::Refused(Refusal::Access));
        }
        Ok(Permit {
            grant_id: "synthetic".into(),
        })
    }
}

#[test]
fn n36_pending_continuation_revalidates_older_covered_history() {
    let fixture = Fixture::new(|policy| {
        clean(policy);
        policy["sources"][0]["limits"]["pages"] = json!(2);
    });
    super::history_tests::historical(&fixture, 1, true, "reader");
    super::history_tests::cover_history(&fixture);
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    assert_eq!(
        fixture.sync_window(Mode::Incremental, now).status,
        Status::Partial
    );
    assert!(
        !fixture
            .site
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|path| path.starts_with("/docs/legacy-"))
    );
    let second = fixture.sync_window(Mode::Incremental, now + Duration::from_secs(5));
    assert_eq!(second.status, Status::Complete, "{second:?}");
    assert_eq!(
        fixture
            .site
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.starts_with("/docs/legacy-"))
            .count(),
        1,
        "older covered observation incorrectly reused in pending continuation"
    );
    fixture.finish();
}
