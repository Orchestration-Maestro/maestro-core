//! N36 production sync revalidation with an injected frozen verification clock.
use super::{
    flow_edges::{Fixture, clean},
    history_tests::historical,
    inspect::inspect,
};
use maestro_acquisition::lifecycle::full::{Mode, changed};
use maestro_kernel::{
    acquisition::{
        CaptureEnvelope, Captures, Enumeration, Frontier, Handle, Partitions, Receipts, Status,
    },
    artifact::Digest,
    scope::Scope,
    store::Database,
};
use serde_json::json;
use std::time::{Duration, UNIX_EPOCH};

#[test]
fn n36_first_incremental_full_repeat_and_expired_history_refresh() {
    let fixture = Fixture::new(clean);
    historical(&fixture, 2, true, "reader");
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let first = fixture.sync_window(Mode::Incremental, now);
    assert_eq!(first.status, Status::Complete, "{first:?}");
    assert_eq!(first.completed.len(), 5);
    assert!(
        first
            .completed
            .iter()
            .all(|entry| entry.reason != "captured_earlier_not_revalidated")
    );
    let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
    let states: Vec<_> = fixture
        .db
        .partition_page(&scope, "notes", None, 1000)
        .unwrap()
        .into_iter()
        .map(|id| fixture.db.partition(&scope, id).unwrap().unwrap())
        .collect();
    let verification: Vec<_> = states
        .iter()
        .flat_map(|state| &state.batches)
        .filter(|batch| batch.partition.kind == Enumeration::Verification)
        .flat_map(|batch| &batch.items)
        .collect();
    let links: Vec<_> = states
        .iter()
        .flat_map(|state| &state.batches)
        .filter(|batch| batch.partition.kind == Enumeration::Links)
        .collect();
    assert!(!links.is_empty(), "no real Links evidence exercised");
    for batch in links {
        let known = batch.parent_keys.as_ref().unwrap();
        let verified = verification
            .iter()
            .find(|item| item.keys.representation == known.representation)
            .unwrap();
        assert_eq!(verified.keys.links, None);
        assert!(
            !changed(known, &verified.keys),
            "same captured bytes changed only because verification has unknown links"
        );
    }
    let count = fixture.site.requests.lock().unwrap().len();
    let repeat = fixture.sync_window(Mode::Incremental, now + Duration::from_secs(1));
    assert_eq!(repeat.status, Status::Complete, "{repeat:?}");
    assert_eq!(
        fixture.site.requests.lock().unwrap().len(),
        count,
        "covered repeat fetched"
    );
    let expired = fixture.sync_window(Mode::Incremental, now + Duration::from_secs(2));
    assert_eq!(expired.status, Status::Complete, "{expired:?}");
    assert!(
        fixture.site.requests.lock().unwrap().len() > count,
        "old verification was never refreshed"
    );
    let full = fixture.sync_window(Mode::Full, now + Duration::from_secs(2));
    assert_eq!(full.status, Status::Complete, "{full:?}");
    assert!(
        full.completed
            .iter()
            .all(|entry| entry.reason != "captured_earlier_not_revalidated")
    );
    assert_eq!(
        inspect(&fixture.db, "reader", full.receipt.unwrap()).unwrap(),
        full
    );
    fixture.finish();
}

#[test]
fn n36_pending_window_never_advances_and_coverage_survives_restart() {
    let fixture = Fixture::new(|policy| {
        clean(policy);
        policy["sources"][0]["limits"]["pages"] = json!(2);
    });
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    let report = fixture.sync_window(Mode::Full, now);
    assert_eq!(report.status, Status::Partial, "{report:?}");
    let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
    let reopened = Database::open_in(&fixture.root).unwrap();
    let partitions = reopened
        .partition_page(&scope, "notes", None, 1000)
        .unwrap();
    let verification: Vec<_> = partitions
        .into_iter()
        .map(|id| reopened.partition(&scope, id).unwrap().unwrap())
        .filter(|state| state.batches.first().unwrap().partition.kind == Enumeration::Verification)
        .collect();
    assert!(!verification.is_empty());
    assert!(
        verification.iter().all(|state| state.accepted.is_none()),
        "partial window advanced"
    );
    assert_eq!(
        verification
            .iter()
            .map(|state| state.batches.first().unwrap().items.len())
            .sum::<usize>(),
        3
    );
    assert!(
        verification
            .iter()
            .all(|state| !state.batches.first().unwrap().stable)
    );
    assert_eq!(
        verification
            .iter()
            .flat_map(|state| &state.batches.first().unwrap().items)
            .filter(|item| item.keys.representation.is_none())
            .count(),
        1,
        "pending unobserved bytes acquired invented evidence"
    );
    drop(reopened);
    fixture.finish();
}

#[test]
fn n36_full_rechecks_reply_edits_deletions_and_link_only_discovery() {
    let mut fixture = Fixture::new(clean);
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    assert_eq!(
        fixture.sync_window(Mode::Full, now).status,
        Status::Complete
    );
    let scope: Scope = "workspace/default/collection/garden".parse().unwrap();
    let scopes = fixture.db.visible("reader").unwrap();
    let item = Frontier::page(&fixture.db, &scopes, "notes", None, 1000)
        .unwrap()
        .into_iter()
        .find(|item| item.request.fetch_identity.ends_with("/start"))
        .unwrap();
    let old = fixture.db.capture_for(&scope, &item).unwrap().unwrap();
    fixture.site.revision = super::flow_tests::Revision::Changed;
    let changed = fixture.sync_window(Mode::Full, now + Duration::from_secs(1));
    assert_eq!(changed.status, Status::Complete, "{changed:?}");
    assert_eq!(changed.completed.len(), 4, "new link not fetched");
    assert!(
        fixture
            .site
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|path| path == "/docs/new")
    );
    assert!(
        fixture.db.read("reader", old).unwrap().is_some(),
        "history overwritten"
    );
    let updated = Frontier::page(&fixture.db, &scopes, "notes", None, 1000)
        .unwrap()
        .into_iter()
        .find(|row| row.id == item.id)
        .unwrap();
    assert_ne!(
        fixture.db.capture_for(&scope, &updated).unwrap().unwrap(),
        old
    );
    fixture.finish();
}

#[test]
fn n36_verification_keys_refuse_foreign_capture_provenance() {
    let fixture = Fixture::new(clean);
    let now = UNIX_EPOCH + Duration::from_secs(2_000_000);
    assert_eq!(
        fixture.sync_window(Mode::Full, now).status,
        Status::Complete
    );
    let item = Frontier::page(&fixture.db, &fixture.scopes, "notes", None, 1)
        .unwrap()
        .remove(0);
    let handle = fixture
        .db
        .capture_for(&fixture.scope, &item)
        .unwrap()
        .unwrap();
    let bytes = fixture.db.read("reader", handle).unwrap().unwrap();
    let envelope: CaptureEnvelope = serde_json::from_slice(bytes.bytes()).unwrap();
    for field in 0..5 {
        let mut observed = envelope.clone();
        match field {
            0 => observed.item = Handle::new(),
            1 => observed.source = "other".into(),
            2 => observed.authorization_context = Digest::of(b"other principal"),
            3 => observed.profile = Digest::of(b"other profile"),
            _ => {}
        }
        let result = super::sync_keys::observed_keys(&observed, &item);
        assert_eq!(
            result.is_ok(),
            field == 4,
            "capture provenance mismatch {field} admitted"
        );
        if let Ok(keys) = result {
            assert_eq!(keys.representation, Some(envelope.artifact.clone()));
            assert_eq!(keys.links, None);
        }
    }
    fixture.finish();
}
