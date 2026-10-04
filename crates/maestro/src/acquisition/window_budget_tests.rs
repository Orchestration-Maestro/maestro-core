//! Verification partition ceilings and per-run dispatch attempt rebasing.
use super::{
    flow_edges::{Fixture, clean},
    window_edges::windows,
};
use maestro_acquisition::lifecycle::full::Mode;
use maestro_kernel::acquisition::{CaptureEnvelope, Captures, Frontier, Receipts, Status};
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
fn n36_verification_respects_source_and_aggregate_partition_caps() {
    for (source, aggregate) in [(1, 1), (1, 2), (2, 1), (2, 2)] {
        let fixture = Fixture::new(|policy| {
            clean(policy);
            policy["sources"][0]["discovery"] = json!([]);
            policy["sources"][0]["seeds"] = json!([
                "https://garden.example/docs/start",
                "https://garden.example/docs/allowed",
                "https://garden.example/docs/final"
            ]);
            policy["sources"][0]["limits"]["partitions"] = json!(source);
            policy["aggregate_limits"]["partitions"] = json!(aggregate);
        });
        let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
        let report = fixture.sync_window(Mode::Full, now);
        let enough = source == 2 && aggregate == 2;
        assert_eq!(
            report.status,
            if enough {
                Status::Complete
            } else {
                Status::Partial
            },
            "{report:?}"
        );
        let coverage = windows(&fixture);
        assert_eq!(coverage.len(), source.min(aggregate));
        assert!(
            coverage
                .iter()
                .all(|state| state.accepted.is_some() == enough)
        );
        if !enough {
            assert!(report.pending.iter().any(|entry| {
                entry.reason == "inventory_or_partition_limit"
                    && entry
                        .partition_limit
                        .as_ref()
                        .is_some_and(|limit| limit.chunks == 2 && limit.ceiling == 1)
            }));
        }
        if !enough {
            assert!(
                report
                    .text()
                    .contains("verification needs 2 chunks; limits.partitions ceiling 1")
            );
        }
        let receipt = fixture
            .db
            .inspect("reader", report.receipt.unwrap())
            .unwrap()
            .unwrap();
        let bytes: u64 = coverage
            .iter()
            .flat_map(|state| &state.batches)
            .map(|batch| serde_json::to_vec(batch).unwrap().len() as u64)
            .sum();
        let captures: u64 = Frontier::page(&fixture.db, &fixture.scopes, "notes", None, 100)
            .unwrap()
            .iter()
            .map(|item| {
                let handle = fixture
                    .db
                    .capture_for(&fixture.scope, item)
                    .unwrap()
                    .unwrap();
                let retained = fixture.db.read("reader", handle).unwrap().unwrap();
                let envelope: CaptureEnvelope = serde_json::from_slice(retained.bytes()).unwrap();
                fixture.db.capture_bytes(&envelope).unwrap()
            })
            .sum();
        assert_eq!(
            receipt.budget.staging_bytes,
            captures + bytes,
            "coverage staging not charged exactly"
        );
        fixture.finish();
    }
}

#[test]
fn n36_repeated_full_sweeps_rebase_attempt_budget_without_resetting_epochs() {
    let fixture = Fixture::new(|policy| {
        clean(policy);
        policy["sources"][0]["limits"]["requests"] = json!(4);
    });
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    for index in 0..5 {
        let report = fixture.sync_window(Mode::Full, now + Duration::from_secs(index));
        assert_eq!(report.status, Status::Complete, "sweep {index}: {report:?}");
        let rows = Frontier::page(&fixture.db, &fixture.scopes, "notes", None, 100).unwrap();
        assert!(
            rows.iter()
                .all(|item| item.epoch == index + 1 && item.attempts == index + 1),
            "attempt history or monotonic fencing lost: {rows:?}"
        );
    }
    fixture.finish();
}

#[test]
fn n36_staging_hold_after_chunk_one_continues_without_false_watermark_or_fetch() {
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let generous = verification_fixture(10_000_000);
    let complete = generous.sync_window(Mode::Full, now);
    assert_eq!(complete.status, Status::Complete);
    let total = generous
        .db
        .inspect("reader", complete.receipt.unwrap())
        .unwrap()
        .unwrap()
        .budget
        .staging_bytes;
    let last = windows(&generous)
        .into_iter()
        .flat_map(|chunk| chunk.batches)
        .find(|batch| batch.verification_final)
        .unwrap();
    let second_bytes = serde_json::to_vec(&last).unwrap().len() as u64;
    let capture_bytes = total
        - windows(&generous)
            .iter()
            .flat_map(|chunk| &chunk.batches)
            .map(|batch| serde_json::to_vec(batch).unwrap().len() as u64)
            .sum::<u64>();
    generous.finish();
    // Opaque IDs can reorder unequal URL lengths within a millisecond.
    let fixture = verification_fixture(total - second_bytes + 64);
    let first = fixture.sync_window(Mode::Full, now);
    assert_eq!(first.status, Status::Partial, "{first:?}");
    assert!(first.pending.iter().any(|entry| entry.reason == "budget"));
    let chunks = windows(&fixture);
    assert_eq!(chunks.len(), 1, "hold must follow first checkpoint");
    assert!(chunks.first().unwrap().accepted.is_some());
    assert!(
        !chunks
            .first()
            .unwrap()
            .batches
            .first()
            .unwrap()
            .verification_final
    );
    let receipt = fixture
        .db
        .inspect("reader", first.receipt.unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(receipt.status, Status::Partial);
    assert_eq!(
        receipt.budget.staging_bytes,
        capture_bytes
            + serde_json::to_vec(chunks.first().unwrap().batches.first().unwrap())
                .unwrap()
                .len() as u64
    );
    assert!(
        fixture
            .db
            .unfinalized(
                "reader",
                &maestro_kernel::acquisition::UnfinalizedPage {
                    scope: &fixture.scope,
                    now: SystemTime::now(),
                    after: None,
                    limit: 100,
                }
            )
            .unwrap()
            .is_empty(),
        "handled cap retained a crash candidate"
    );
    let requests = fixture.site.requests.lock().unwrap().len();
    let second = fixture.sync_window(Mode::Incremental, now + Duration::from_secs(5));
    assert_eq!(second.status, Status::Complete, "{second:?}");
    assert_eq!(fixture.site.requests.lock().unwrap().len(), requests);
    assert!(
        windows(&fixture)
            .iter()
            .filter(|chunk| chunk.batches.first().unwrap().partition.run == second.run.unwrap())
            .all(|chunk| chunk.accepted.as_ref().unwrap().watermark == 2_000_000_000)
    );
    fixture.finish();
}

/// No discovery costs obscure the two local verification checkpoint allocations.
fn verification_fixture(staging: u64) -> Fixture {
    Fixture::new(|policy| {
        clean(policy);
        policy["sources"][0]["discovery"] = json!([]);
        policy["sources"][0]["seeds"] = json!([
            "https://garden.example/docs/start",
            "https://garden.example/docs/allowed",
            "https://garden.example/docs/final"
        ]);
        policy["sources"][0]["limits"]["staging_bytes"] = json!(staging);
        policy["aggregate_limits"]["staging_bytes"] = json!(staging);
    })
}

#[test]
fn n36_unfinalized_receipts_exclude_live_lease_and_recheck_scope_and_bounds() {
    use super::{command::new_receipt, output::Report};
    use maestro_kernel::acquisition::{Handle, LeaseRequest, ReceiptError, UnfinalizedPage};
    let fixture = Fixture::new(clean);
    let inputs = fixture
        .db
        .retain(&fixture.scope, b"synthetic pending run", &[])
        .unwrap();
    let mut report = Report::new();
    report.run = Some(Handle::new());
    report.receipt = Some(Handle::new());
    let receipt = new_receipt(&report, inputs).unwrap();
    fixture.db.begin(&fixture.scope, &receipt).unwrap();
    let now = UNIX_EPOCH + Duration::from_secs(4_000_000_000);
    let holder = receipt.attempt.to_string();
    let writer = fixture
        .db
        .lease_source(
            "notes",
            &fixture.scope,
            LeaseRequest {
                holder: &holder,
                now,
                term: Duration::from_secs(30),
            },
        )
        .unwrap();
    let mut page = UnfinalizedPage {
        scope: &fixture.scope,
        now,
        after: None,
        limit: 1,
    };
    assert!(
        fixture.db.unfinalized("reader", &page).unwrap().is_empty(),
        "live run adopted as crashed"
    );
    page.now = now + Duration::from_secs(30);
    assert_eq!(
        fixture.db.unfinalized("reader", &page).unwrap(),
        vec![receipt.clone()]
    );
    assert!(
        fixture
            .db
            .unfinalized("stranger", &page)
            .unwrap()
            .is_empty()
    );
    page.after = Some(receipt.attempt);
    assert!(fixture.db.unfinalized("reader", &page).unwrap().is_empty());
    page.limit = 0;
    assert_eq!(
        fixture.db.unfinalized("reader", &page),
        Err(ReceiptError::Invalid)
    );
    fixture
        .db
        .release_source(&writer, now + Duration::from_secs(1))
        .unwrap();
    fixture.finish();
}

#[test]
fn n36_verification_partition_usage_is_shared_across_sources() {
    use maestro_kernel::acquisition::Partitions;
    for cap in [1, 2] {
        let fixture = Fixture::new(|policy| {
            clean(policy);
            policy["sources"][0]["discovery"] = json!([]);
            policy["sources"][0]["seeds"] = json!(["https://garden.example/docs/allowed"]);
            let mut second = policy["sources"][0].clone();
            second["id"] = json!("other-notes");
            second["selectors"][0]["source_id"] = json!("other-notes");
            second["seeds"] = json!(["https://garden.example/docs/final"]);
            policy["sources"].as_array_mut().unwrap().push(second);
            policy["aggregate_limits"]["partitions"] = json!(cap);
        });
        let report = fixture.sync_window(Mode::Full, UNIX_EPOCH + Duration::from_secs(2_000_000));
        assert_eq!(
            report.status,
            if cap == 1 {
                Status::Partial
            } else {
                Status::Complete
            },
            "{report:?}"
        );
        let count: usize = ["notes", "other-notes"]
            .iter()
            .map(|source| {
                fixture
                    .db
                    .partition_page(&fixture.scope, source, None, 100)
                    .unwrap()
                    .len()
            })
            .sum();
        assert_eq!(count, cap);
        fixture.finish();
    }
}
