//! Production retained-capture paths cannot bypass current authority on resume.
use super::flow_fixture::{Fixture, clean};
use maestro_acquisition::{
    Refusal,
    lifecycle::full::Mode,
    policy::authority::{
        Authority, AuthorityRefusal, Operation, Permit, Target, UnqualifiedAuthority,
    },
};
use maestro_kernel::acquisition::{InventoryPage, ItemDisposition, Receipts, Status};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[test]
fn n37_cli_current_authority_holds_previously_completed_captures() {
    let fixture = Fixture::new(clean);
    let time = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let first = fixture.sync_window(Mode::Full, time);
    assert_eq!(first.status, Status::Complete);
    let before = fixture.site.requests.lock().unwrap().len();
    let resumed = fixture
        .run_window(
            2,
            Mode::Incremental,
            time + Duration::from_secs(1),
            &UnqualifiedAuthority,
        )
        .unwrap();
    assert!(
        resumed.completed.is_empty(),
        "revoked captures counted complete: {resumed:?}"
    );
    assert_ne!(resumed.status, Status::Complete);
    assert_eq!(fixture.site.requests.lock().unwrap().len(), before);
    let receipt = fixture
        .db
        .inspect("reader", resumed.receipt.unwrap())
        .unwrap()
        .unwrap();
    assert_ne!(receipt.status, Status::Complete);
    let mut pending = 0;
    for handle in receipt.inventories {
        let bytes = fixture.db.read("reader", handle).unwrap().unwrap();
        let page: InventoryPage = serde_json::from_slice(bytes.bytes()).unwrap();
        for item in page.items {
            assert_eq!(item.disposition, ItemDisposition::Pending);
            pending += 1;
        }
    }
    assert_eq!(pending, 3);
    fixture.finish();
}

#[test]
fn n37_cli_prepared_crash_reuses_identical_inputs_without_fetch() {
    prepared_crash(false);
}
#[test]
fn n37_cli_prepared_crash_refetches_changed_policy_without_adoption() {
    prepared_crash(true);
}
/// Same crash boundary, with one deliberate frozen-policy difference.
fn prepared_crash(changed: bool) {
    use super::{command::resolve, flow_tests::fixture_with};
    use maestro_acquisition::Principal;
    use maestro_kernel::artifact::Digest;
    use rusqlite::Connection;
    use serde_json::json;
    use std::env;
    let edit = |value: &mut serde_json::Value| {
        clean(value);
        value["sources"][0]["discovery"] = json!([]);
    };
    let mut fixture = Fixture::mapped(edit, "1000", "reader");
    let sql = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    sql.execute_batch(
        "CREATE TRIGGER n37_crash BEFORE UPDATE OF capture ON acquisition_frontier
            WHEN NEW.capture IS NOT NULL BEGIN SELECT RAISE(ABORT, 'synthetic crash'); END;",
    )
    .unwrap();
    let time = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let interrupted = fixture
        .run_window(2, Mode::Full, time, &super::flow_tests::Grants::default())
        .unwrap();
    assert_eq!(interrupted.status, Status::Failed);
    sql.execute_batch("DROP TRIGGER n37_crash;").unwrap();
    drop(sql);
    let before = fixture
        .site
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|path| path.as_str() == "/docs/start")
        .count();
    assert_eq!(before, 1);
    if !changed {
        let held = fixture
            .run_window(
                2,
                Mode::Incremental,
                time + Duration::from_secs(1),
                &UnqualifiedAuthority,
            )
            .unwrap();
        assert!(
            held.completed.is_empty(),
            "unacknowledged capture adopted without authority"
        );
        assert!(
            held.pending
                .iter()
                .any(|entry| entry.reason == "current_resume_admission"),
            "prepared capture bypassed the early admission guard: {held:?}"
        );
    }
    if changed {
        let (collection, files) = fixture_with(|value| {
            edit(value);
            value["sources"][0]["sync"]["overlap_ms"] = json!(1);
        });
        let principal = Principal {
            id: "reader",
            platform: env::consts::OS,
            scopes: &fixture.scopes,
        };
        fixture.policy = resolve(&files, &collection, &principal).unwrap();
        fixture.collection = Digest::of(&serde_json::to_vec(&collection).unwrap());
    }
    let resumed = fixture.sync_window(Mode::Incremental, time + Duration::from_secs(1));
    assert_eq!(resumed.status, Status::Complete, "{changed}: {resumed:?}");
    let after = fixture
        .site
        .requests
        .lock()
        .unwrap()
        .iter()
        .filter(|path| path.as_str() == "/docs/start")
        .count();
    assert_eq!(
        after - before,
        usize::from(changed),
        "prepared bytes adopted/refetched incorrectly (changed={changed})"
    );
    assert_eq!(resumed.completed.len(), 1);
    fixture.finish();
}

/// Exact final-target revocation leaves the originally requested redirect admitted.
#[derive(Debug)]
struct WithoutFinal;
impl Authority for WithoutFinal {
    fn decide(
        &self,
        _: &str,
        _: Operation,
        target: &Target,
        _: SystemTime,
    ) -> Result<Permit, AuthorityRefusal> {
        if target.resource == "https://garden.example/docs/final" {
            return Err(Refusal::Access.into());
        }
        Ok(Permit {
            grant_id: "synthetic".into(),
        })
    }
}
#[test]
fn n37_revoked_redirect_final_holds_completed_capture() {
    let fixture = Fixture::new(|value| {
        value["sources"][0]["seeds"] = serde_json::json!(["https://garden.example/docs/redirect"]);
        value["sources"][0]["discovery"] = serde_json::json!([]);
    });
    let time = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let first = fixture.sync_window(Mode::Full, time);
    assert_eq!(first.status, Status::Complete, "{first:?}");
    assert_eq!(first.completed.len(), 1);
    let before = fixture.site.requests.lock().unwrap().len();
    let resumed = fixture
        .run_window(
            2,
            Mode::Incremental,
            time + Duration::from_secs(1),
            &WithoutFinal,
        )
        .unwrap();
    assert_eq!(fixture.site.requests.lock().unwrap().len(), before);
    assert!(
        resumed.completed.is_empty(),
        "revoked final target reused: {resumed:?}"
    );
    assert_ne!(resumed.status, Status::Complete);
    fixture.finish();
}
