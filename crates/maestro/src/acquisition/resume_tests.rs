//! Permanent N14 interrupted-depth and cross-source aggregate regressions.
use super::{
    command::resolve,
    flow_edges::{Fixture, clean},
    flow_tests::fixture_with,
};
use maestro_acquisition::{Principal, lifecycle::full::Mode};
use maestro_kernel::{
    acquisition::{
        Enumeration, Frontier, LeaseRequest, NewItem, Partitions as _, Receipts as _, Status,
    },
    artifact::Digest,
};
use serde_json::{Value, json};
use std::{
    env,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

#[test]
fn n14_review_checkpoint_depth_survives_resume() {
    let mut fixture = Fixture::new(|value| {
        clean(value);
        value["sources"][0]["limits"]["requests"] = json!(2);
    });
    let first = fixture.sync();
    assert_eq!(first.status, Status::Partial, "{first:?}");
    assert_eq!(first.completed.len(), 1);
    let scope = "workspace/default/collection/garden".parse().unwrap();
    let checkpoint = *fixture
        .db
        .partition_page(&scope, "notes", None, 1000)
        .unwrap()
        .first()
        .unwrap();
    let scopes = fixture.db.visible("reader").unwrap();
    let principal = Principal {
        id: "reader",
        platform: env::consts::OS,
        scopes: &scopes,
    };
    assert!(
        fixture
            .db
            .partition_page(&scope, "other-source", None, 1000)
            .unwrap()
            .is_empty()
    );
    let other_scope = "workspace/default/collection/other".parse().unwrap();
    assert!(
        fixture
            .db
            .partition_page(&other_scope, "notes", None, 1000)
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .db
            .partition_page(&scope, "notes", Some(checkpoint), 1000)
            .unwrap()
            .into_iter()
            .all(|id| fixture
                .db
                .partition(&scope, id)
                .unwrap()
                .unwrap()
                .batches
                .first()
                .unwrap()
                .partition
                .kind
                != Enumeration::Links)
    );
    assert!(fixture.db.partition_page(&scope, "notes", None, 0).is_err());
    let (collection, files) = fixture_with(clean);
    fixture.policy = resolve(&files, &collection, &principal).unwrap();
    fixture.collection = Digest::of(&serde_json::to_vec(&collection).unwrap());
    let second = fixture.sync();
    assert_eq!(
        second.status,
        Status::Complete,
        "checkpoint child depths lost: {second:?}"
    );
    assert_eq!(second.completed.len(), 3);
    assert!(
        fixture
            .db
            .partition(&scope, checkpoint)
            .unwrap()
            .unwrap()
            .accepted
            .is_some(),
        "resumed child captures did not reconcile the original partition"
    );
    assert_eq!(
        fixture
            .site
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.as_str() == "/docs/start")
            .count(),
        1
    );
    fixture.finish();
}

#[test]
fn n14_review_page_budget_is_aggregate_across_sources() {
    let fixture = Fixture::new(|value| {
        clean(value);
        value["sources"][0]["discovery"] = json!([]);
        value["sources"][0]["seeds"] = json!(["https://garden.example/docs/allowed"]);
        let mut second = value["sources"][0].clone();
        second["id"] = json!("other-notes");
        second["selectors"][0]["source_id"] = json!("other-notes");
        second["seeds"] = json!(["https://garden.example/docs/final"]);
        value["sources"].as_array_mut().unwrap().push(second);
        value["aggregate_limits"]["pages"] = json!(1);
    });
    let report = fixture.sync();
    assert!(
        report.completed.len() <= 1,
        "aggregate page ceiling exceeded: {report:?}"
    );
    assert_eq!(report.status, Status::Partial);
    fixture.finish();
}

/// Two separately bound sources still share the single sync budget and run slot.
fn two_sources(value: &mut Value) {
    clean(value);
    value["sources"][0]["discovery"] = json!([]);
    value["sources"][0]["seeds"] = json!(["https://garden.example/docs/allowed"]);
    let mut second = value["sources"][0].clone();
    second["id"] = json!("other-notes");
    second["selectors"][0]["source_id"] = json!("other-notes");
    second["seeds"] = json!(["https://garden.example/docs/final"]);
    value["sources"].as_array_mut().unwrap().push(second);
}

#[test]
fn n14_two_sources_complete_with_one_run_slot_and_ample_budgets() {
    let fixture = Fixture::new(two_sources);
    let report = fixture.sync();
    assert_eq!(report.status, Status::Complete, "{report:?}");
    assert_eq!(report.completed.len(), 2);
    fixture.finish();
}

#[test]
fn n14_requests_and_response_bytes_are_aggregate_across_sources() {
    let first = Fixture::new(|value| {
        clean(value);
        value["sources"][0]["discovery"] = json!([]);
        value["sources"][0]["seeds"] = json!(["https://garden.example/docs/allowed"]);
    });
    let report = first.sync();
    let bytes = first
        .db
        .inspect("reader", report.receipt.unwrap())
        .unwrap()
        .unwrap()
        .budget
        .response_bytes;
    assert!(bytes > 0);
    first.finish();
    for (field, cap) in [
        ("requests", 2),
        ("wire_bytes", bytes),
        ("expanded_bytes", bytes),
    ] {
        let fixture = Fixture::new(|value| {
            two_sources(value);
            if field == "expanded_bytes" {
                value["aggregate_limits"]["decode"][field] = json!(bytes);
            } else {
                value["aggregate_limits"][field] = json!(cap);
            }
        });
        let report = fixture.sync();
        assert_eq!(report.status, Status::Partial, "{field}: {report:?}");
        assert_eq!(report.completed.len(), 1, "{field}: {report:?}");
        assert!(
            report
                .pending
                .iter()
                .any(|entry| entry.reason == "aggregate_budget"),
            "{field} did not hold before dispatch: {report:?}"
        );
        assert!(
            !fixture
                .site
                .requests
                .lock()
                .unwrap()
                .iter()
                .any(|path| path == "/docs/final")
        );
        fixture.finish();
    }
}

#[test]
fn n14_second_source_staging_includes_retained_first_source_bytes() {
    let ample = Fixture::new(two_sources);
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let report = ample.sync_window(Mode::Full, now);
    let total = ample
        .db
        .inspect("reader", report.receipt.unwrap())
        .unwrap()
        .unwrap()
        .budget
        .staging_bytes;
    assert_eq!(report.completed.len(), 2);
    let verification_bytes = ample
        .db
        .partition_page(&ample.scope, "other-notes", None, 100)
        .unwrap()
        .into_iter()
        .map(|id| {
            ample
                .db
                .partition(&ample.scope, id)
                .unwrap()
                .unwrap()
                .batches
                .into_iter()
                .map(|batch| serde_json::to_vec(&batch).unwrap().len() as u64)
                .sum::<u64>()
        })
        .sum::<u64>();
    ample.finish();
    for (ceiling, expected, complete) in [
        (total - verification_bytes - 1, 1, false),
        (total - 1, 2, false),
        (total, 2, true),
    ] {
        let fixture = Fixture::new(|value| {
            two_sources(value);
            value["aggregate_limits"]["staging_bytes"] = json!(ceiling);
        });
        let report = fixture.sync_window(Mode::Full, now);
        assert_eq!(
            report.completed.len(),
            expected,
            "staging {ceiling}: {report:?}"
        );
        assert_eq!(
            report.status,
            if complete {
                Status::Complete
            } else {
                Status::Partial
            }
        );
        let receipt = fixture
            .db
            .inspect("reader", report.receipt.unwrap())
            .unwrap()
            .unwrap();
        assert!(receipt.budget.staging_bytes <= ceiling);
        if !complete {
            assert!(!report.pending.is_empty());
            assert!(
                fixture
                    .db
                    .partition_page(&fixture.scope, "other-notes", None, 100)
                    .unwrap()
                    .into_iter()
                    .all(|id| fixture
                        .db
                        .partition(&fixture.scope, id)
                        .unwrap()
                        .unwrap()
                        .accepted
                        .is_none()),
                "held second-source verification advanced watermark"
            );
        }
        fixture.finish();
    }
}

#[test]
fn n14_second_source_keeps_its_local_staging_bound() {
    let fixture = Fixture::new(|value| {
        two_sources(value);
        value["sources"][1]["limits"]["staging_bytes"] = json!(1);
    });
    let report = fixture.sync();
    assert_eq!(report.completed.len(), 1, "{report:?}");
    assert_eq!(report.status, Status::Partial);
    fixture.finish();
}

#[test]
fn n14_inventory_overflow_retains_exact_count_and_holds_window() {
    let fixture = Fixture::new(|value| {
        clean(value);
        value["sources"][0]["limits"]["pages"] = json!(1);
    });
    for _ in 0..4 {
        let report = fixture.sync();
        assert_eq!(report.status, Status::Partial, "{report:?}");
        assert_eq!(report.overflow, 1);
        assert!(
            report
                .text()
                .contains("1 links pending: inventory limit reached")
        );
        let scope = "workspace/default/collection/garden".parse().unwrap();
        for id in fixture
            .db
            .partition_page(&scope, "notes", None, 1000)
            .unwrap()
        {
            let state = fixture.db.partition(&scope, id).unwrap().unwrap();
            if state
                .batches
                .iter()
                .any(|batch| batch.parent_depth == Some(0))
            {
                assert!(
                    state.accepted.is_none(),
                    "over-bound parent advanced a watermark"
                );
            }
        }
    }
    let paths = fixture.site.requests.lock().unwrap().clone();
    for path in ["/docs/start", "/docs/allowed", "/docs/final"] {
        assert_eq!(
            paths
                .iter()
                .filter(|actual| actual.as_str() == path)
                .count(),
            1,
            "{path}: {paths:?}"
        );
    }
    fixture.finish();
}

#[test]
fn n14_full_checkpoint_never_commits_while_an_inventoried_child_is_held() {
    let mut fixture = Fixture::new(|value| {
        clean(value);
        value["sources"][0]["limits"]["pages"] = json!(2);
    });
    let first = fixture.sync();
    assert_eq!(first.status, Status::Partial);
    assert_eq!(first.overflow, 0);
    let scope = "workspace/default/collection/garden".parse().unwrap();
    let ids = fixture
        .db
        .partition_page(&scope, "notes", None, 1000)
        .unwrap();
    let parent = ids
        .into_iter()
        .find(|id| {
            fixture
                .db
                .partition(&scope, *id)
                .unwrap()
                .unwrap()
                .batches
                .iter()
                .any(|batch| batch.parent_depth == Some(0))
        })
        .unwrap();
    let state = fixture.db.partition(&scope, parent).unwrap().unwrap();
    assert_eq!(state.batches.len(), 1);
    assert_eq!(
        state.batches.first().unwrap().items.len(),
        2,
        "full eligible inventory was not checkpointed"
    );
    assert_eq!(state.pending, 1);
    assert!(state.accepted.is_none());
    fixture.site.failure = true;
    let held = fixture.sync();
    assert_eq!(held.status, Status::Partial);
    let state = fixture.db.partition(&scope, parent).unwrap().unwrap();
    assert_eq!(state.pending, 1);
    assert!(
        state.accepted.is_none(),
        "held historical child advanced the watermark"
    );
    fixture.finish();
}

#[test]
fn n14_capped_fetches_resume_full_checkpoint_without_refetch() {
    let capped = Fixture::new(|value| {
        clean(value);
        value["sources"][0]["limits"]["pages"] = json!(2);
    });
    let report = capped.sync();
    assert_eq!(report.status, Status::Partial, "{report:?}");
    assert_eq!(report.overflow, 0);
    assert_eq!(
        report
            .pending
            .iter()
            .filter(|entry| entry.reason == "page_budget")
            .count(),
        1
    );
    let second = capped.sync();
    assert_eq!(
        second.status,
        Status::Complete,
        "second capped run did not continue: {second:?}"
    );
    assert_eq!(
        capped
            .site
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.as_str() == "/docs/start")
            .count(),
        1,
        "retained parent fetched twice"
    );
    capped.finish();
    let exact = Fixture::new(|value| {
        clean(value);
        value["sources"][0]["limits"]["pages"] = json!(3);
    });
    let report = exact.sync();
    assert_eq!(report.overflow, 0);
    assert_eq!(report.status, Status::Complete, "{report:?}");
    exact.finish();
}

#[test]
fn n14_review_historical_policy_denial_is_discarded() {
    for url in [
        "https://garden.example/docs/private",
        "https://garden.example/outside",
    ] {
        let fixture = Fixture::new(clean);
        let scope = "workspace/default/collection/garden".parse().unwrap();
        let now = SystemTime::now();
        let writer = fixture
            .db
            .lease_source(
                "notes",
                &scope,
                LeaseRequest {
                    holder: "earlier",
                    now,
                    term: Duration::from_secs(60),
                },
            )
            .unwrap();
        fixture
            .db
            .enqueue(
                &writer,
                &NewItem {
                    fetch_identity: url.into(),
                    authorization_context: Digest::of(b"reader"),
                    representation_profile: fixture
                        .policy
                        .policy()
                        .sources
                        .first()
                        .unwrap()
                        .acquisition_profile
                        .digest
                        .clone(),
                },
                now,
            )
            .unwrap();
        fixture.db.release_source(&writer, now).unwrap();
        let report = fixture.sync();
        let reference = Digest::of(url.as_bytes());
        assert!(
            report
                .discarded
                .iter()
                .any(|entry| entry.reference == reference && entry.reason == "policy_denial"),
            "historical exclusion misclassified: {report:?}"
        );
        assert!(
            !report
                .pending
                .iter()
                .any(|entry| entry.reference == reference)
        );
        assert!(
            !fixture
                .site
                .requests
                .lock()
                .unwrap()
                .iter()
                .any(|path| path.as_str() == "/docs/private" || path.as_str() == "/outside")
        );
        fixture.finish();
    }
}
