//! Dispositions and admission must agree with the real durable frontier.
use super::{
    command::preview,
    controls::Controls,
    flow_fixture::{Fixture, clean},
    flow_tests::Grants,
};
use maestro_kernel::{
    acquisition::{Frontier, Reason, Receipts, Status},
    artifact::Digest,
};
use serde_json::json;
use std::time::SystemTime;

#[test]
fn debt_preview_dispositions_keep_denial_separate_from_unknown_robots() {
    let fixture = Fixture::new(|_| {});
    let authority = Grants::default();
    let controls = Controls::new(
        &fixture.policy,
        &authority,
        "reader",
        fixture.scope.as_str(),
    );
    let report = preview(&fixture.policy, &controls, SystemTime::now()).unwrap();
    assert_eq!(
        report.discarded[0].reference,
        Digest::of(b"https://garden.example/docs/private")
    );
    assert_eq!(report.discarded[0].reason, "policy_denial");
    assert_eq!(report.pending[0].reason, "robots_unavailable");
    fixture.finish();
}

#[test]
fn debt_seed_refusals_never_enter_the_durable_frontier() {
    for unknown in [false, true] {
        let fixture = Fixture::new(|policy| {
            if unknown {
                clean(policy);
                policy["sources"][0]["selectors"][0]["media_types"] = json!(["text/html"]);
            } else {
                policy["sources"][0]["seeds"] = json!(["https://garden.example/docs/private"]);
            }
        });
        let report = fixture.sync();
        if unknown {
            let receipt = fixture
                .db
                .inspect("reader", report.receipt.unwrap())
                .unwrap()
                .unwrap();
            assert_eq!(receipt.reason, Reason::Unsupported);
        }
        let entries = if unknown {
            &report.pending
        } else {
            &report.discarded
        };
        assert!(entries.iter().any(|entry| entry.reason
            == if unknown {
                "unresolved_identity"
            } else {
                "policy_denial"
            }));
        assert!(
            Frontier::page(&fixture.db, &fixture.scopes, "notes", None, 100)
                .unwrap()
                .is_empty()
        );
        fixture.finish();
    }
}

#[test]
fn debt_sync_complete_receipt_has_no_unsupported_reason() {
    let fixture = Fixture::new(clean);
    let report = fixture.sync();
    assert_eq!(report.status, Status::Complete);
    let receipt = fixture
        .db
        .inspect("reader", report.receipt.unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(receipt.reason, Reason::None);
    assert_eq!(report.overflow, 0);
    fixture.finish();
}

#[test]
fn debt_revalidation_discovery_hold_is_not_a_revalidated_capture() {
    use super::{command::resolve, flow_tests::fixture_with};
    use maestro_acquisition::{Principal, lifecycle::full::Mode};
    use std::{
        env,
        time::{Duration, UNIX_EPOCH},
    };
    let mut fixture = Fixture::new(clean);
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    assert_eq!(
        fixture.sync_window(Mode::Full, now).status,
        Status::Complete
    );
    let (collection, files) = fixture_with(|value| {
        clean(value);
        value["sources"][0]["limits"]["partitions"] = json!(1);
    });
    fixture.policy = resolve(
        &files,
        &collection,
        &Principal {
            id: "reader",
            platform: env::consts::OS,
            scopes: &fixture.scopes,
        },
    )
    .unwrap();
    let report = fixture.sync_window(Mode::Full, now + Duration::from_secs(1));
    assert_eq!(report.status, Status::Partial);
    assert!(
        report
            .completed
            .iter()
            .any(|entry| entry.reason == "captured_discovery_pending"),
        "{report:?}"
    );
    assert!(
        report
            .completed
            .iter()
            .filter(|entry| entry.reason.starts_with("revalidated_"))
            .count()
            <= 1,
        "{report:?}"
    );
    fixture.finish();
}
