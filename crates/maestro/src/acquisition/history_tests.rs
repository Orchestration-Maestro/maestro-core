//! Cursor paging and the frozen OS-to-kernel mapping use the real durable ports.
use super::{
    flow_edges::{Fixture, clean},
    inspect::inspect,
};
use maestro_kernel::{
    acquisition::{
        CaptureContext, CaptureEnvelope, Captures, DispatchRequest, Frontier, Handle, LeaseRequest,
        NewItem, Receipts, Representation, SafeIdentity, Status, Transport,
    },
    artifact::Digest,
    scope::{LOCAL, Right},
};
use std::{
    collections::BTreeMap,
    time::{Duration, SystemTime},
};

/// Independently authored historical rows, never a network or permission bypass.
pub(super) fn historical(fixture: &Fixture, count: usize, captured: bool, kernel_principal: &str) {
    let now = SystemTime::now();
    let scope = "workspace/default/collection/garden".parse().unwrap();
    let lease = LeaseRequest {
        holder: "earlier",
        now,
        term: Duration::from_hours(1),
    };
    let writer = fixture.db.lease_source("notes", &scope, lease).unwrap();
    let profile = fixture
        .policy
        .policy()
        .sources
        .first()
        .unwrap()
        .acquisition_profile
        .digest
        .clone();
    let context = Digest::of(kernel_principal.as_bytes());
    let inputs = fixture
        .db
        .retain(&scope, b"earlier synthetic frozen inputs", &[])
        .unwrap();
    for index in 0..count {
        let identity = format!("https://garden.example/docs/legacy-{index}");
        let item = fixture
            .db
            .enqueue(
                &writer,
                &NewItem {
                    fetch_identity: identity.clone(),
                    authorization_context: context.clone(),
                    representation_profile: profile.clone(),
                },
                now,
            )
            .unwrap();
        if captured {
            let item = fixture
                .db
                .lease(
                    &writer,
                    item.id,
                    DispatchRequest {
                        lease,
                        max_attempts: 3,
                    },
                )
                .unwrap();
            let capture_context = CaptureContext {
                writer: writer.clone(),
                item,
                now,
            };
            let body = b"%PDF-1.4 earlier synthetic attachment";
            let envelope = CaptureEnvelope {
                schema: "maestro-capture/1".into(),
                source: "notes".into(),
                item: capture_context.item.item.to_string().parse().unwrap(),
                run: Handle::new(),
                requested: SafeIdentity::new(&identity).unwrap(),
                final_identity: SafeIdentity::new(&identity).unwrap(),
                redirects: vec![],
                status: 200,
                headers: BTreeMap::new(),
                declared_media: Some("application/pdf".into()),
                detected_media: None,
                artifact: Digest::of(body),
                length: body.len() as u64,
                observed_ms: 1_000_000_000,
                transport: Transport::Http,
                profile: profile.clone(),
                authorization_context: context.clone(),
                representation: Representation::WireBody,
                parent: None,
                inputs,
                access: inputs,
                decision: inputs,
            };
            let capture = fixture
                .db
                .prepare_capture(&capture_context, &envelope, body, u64::MAX)
                .unwrap();
            fixture
                .db
                .acknowledge_capture(&capture_context, capture.handle)
                .unwrap();
        }
    }
    fixture.db.release_source(&writer, now).unwrap();
}

/// Historical reuse has a prior complete local watermark, not merely retained bytes.
pub(super) fn cover_history(fixture: &Fixture) {
    use maestro_kernel::acquisition::{
        Batch, ChangeKeys, DiscoveredItem, Enumeration, Partition, Partitions, Window,
    };
    let scope = "workspace/default/collection/garden".parse().unwrap();
    let now = SystemTime::now();
    let writer = fixture
        .db
        .lease_source(
            "notes",
            &scope,
            LeaseRequest {
                holder: "coverage",
                now,
                term: Duration::from_secs(30),
            },
        )
        .unwrap();
    let scopes = fixture.db.visible("reader").unwrap();
    let inputs = fixture
        .db
        .retain(&scope, b"synthetic prior full verification", &[])
        .unwrap();
    let mut report = super::output::Report::new();
    report.run = Some(Handle::new());
    report.receipt = Some(Handle::new());
    let mut receipt = super::command::new_receipt(&report, inputs).unwrap();
    fixture.db.begin(&scope, &receipt).unwrap();
    let mut after = None;
    loop {
        let page = Frontier::page(&fixture.db, &scopes, "notes", after, 999).unwrap();
        if page.is_empty() {
            break;
        }
        after = page.last().map(|item| item.id);
        let batch = Batch {
            partition: Partition {
                id: Handle::new(),
                run: receipt.run,
                kind: Enumeration::Verification,
                window: Window {
                    start: 0,
                    end: 1_000_000_000,
                    overlap: 0,
                    skew: 0,
                },
                max_batches: 1,
                max_items: 999,
            },
            cursor: None,
            next: None,
            terminal: true,
            stable: true,
            truncated: false,
            expected: Some(u16::try_from(page.len()).unwrap()),
            items: page
                .into_iter()
                .map(|item| DiscoveredItem {
                    keys: ChangeKeys {
                        revision: None,
                        validator: None,
                        metadata: None,
                        permissions: item.request.authorization_context.clone(),
                        links: None,
                        representation: None,
                    },
                    request: item.request,
                })
                .collect(),
            extractor: None,
            parent_keys: None,
            not_enqueued: vec![],
            inventory_overflow: 0,
            parent_depth: None,
            capture: None,
        };
        fixture.db.checkpoint(&writer, &batch, now).unwrap();
        fixture
            .db
            .commit_partition(&writer, batch.partition.id, now)
            .unwrap();
    }
    receipt.status = Status::Complete;
    fixture.db.finish(&receipt).unwrap();
    fixture.db.release_source(&writer, now).unwrap();
}

#[test]
fn n14_all_three_frontier_pages_and_new_seed_are_seen_with_bounded_fetches() {
    let fixture = Fixture::new(|policy| {
        clean(policy);
        policy["sources"][0]["limits"]["pages"] = serde_json::json!(2);
    });
    historical(&fixture, 5, false, "reader");
    let report = fixture.sync();
    assert_eq!(report.status, Status::Partial, "{report:?}");
    assert_eq!(report.completed.len(), 2);
    assert_eq!(report.pending.len(), 4);
    assert!(
        report
            .pending
            .iter()
            .all(|entry| entry.reason == "page_budget")
    );
    assert_eq!(fixture.site.requests.lock().unwrap().len(), 3); // robots plus two content requests
    assert_eq!(
        inspect(&fixture.db, "reader", report.receipt.unwrap())
            .unwrap()
            .pending
            .len(),
        4
    );
    fixture.finish();
}

#[test]
fn n14_historical_reuse_does_not_spend_new_fetch_slots_and_complete_covers_all_pages() {
    let fixture = Fixture::new(|policy| {
        clean(policy);
        policy["sources"][0]["limits"]["pages"] = serde_json::json!(3);
    });
    historical(&fixture, 5, true, "reader");
    cover_history(&fixture);
    let report = fixture.sync();
    assert_eq!(report.status, Status::Complete, "{report:?}");
    assert_eq!(report.completed.len(), 8);
    assert_eq!(
        report
            .completed
            .iter()
            .filter(|entry| entry.reason == "captured_earlier_not_revalidated")
            .count(),
        5
    );
    assert!(report.pending.is_empty());
    let inspected = inspect(&fixture.db, "reader", report.receipt.unwrap()).unwrap();
    assert_eq!(inspected.completed.len(), 8);
    fixture.finish();
}

#[test]
fn n14_os_user_maps_to_local_kernel_grants_once_and_second_sync_reuses() {
    let fixture = Fixture::mapped(clean, "1000", LOCAL);
    let first = fixture.sync();
    assert_eq!(first.status, Status::Complete);
    let requests = fixture.site.requests.lock().unwrap().len();
    let second = fixture.sync();
    assert_eq!(second.status, Status::Complete);
    assert_eq!(second.completed.len(), 3);
    assert!(
        second
            .completed
            .iter()
            .all(|entry| entry.reason == "captured_earlier_not_revalidated")
    );
    assert_eq!(fixture.site.requests.lock().unwrap().len(), requests);
    assert!(inspect(&fixture.db, "1000", second.receipt.unwrap()).is_err());
    assert_eq!(
        inspect(&fixture.db, LOCAL, second.receipt.unwrap())
            .unwrap()
            .status,
        Status::Complete
    );
    fixture.finish();
}

#[test]
fn n14_kernel_principal_without_current_grants_refuses() {
    let fixture = Fixture::mapped(clean, "1000", LOCAL);
    let scope = "workspace/default/collection/garden".parse().unwrap();
    fixture
        .db
        .revoke(LOCAL, &scope, Right::Read, "owner")
        .unwrap();
    assert!(fixture.try_sync().is_err());
    assert!(fixture.site.requests.lock().unwrap().is_empty());
    fixture.finish();
}

#[test]
fn n14_shared_time_formatter_admits_utc_and_refuses_non_utc() {
    use super::{
        controls::{Controls, millis, request},
        flow_tests::Grants,
    };
    use maestro_acquisition::{
        policy::{decision::admit, format_time},
        transport::robots::{RobotsBinding, RobotsCache},
    };
    use std::time::SystemTime;
    let fixture = Fixture::new(clean);
    let grants = Grants::default();
    let controls = Controls::new(
        &fixture.policy,
        &grants,
        "reader",
        "workspace/default/collection/garden",
    );
    let identity = fixture
        .policy
        .admit_robots("notes", "https://garden.example/robots.txt")
        .unwrap();
    let binding = RobotsBinding::new(&fixture.policy, "notes").unwrap();
    controls.insert(RobotsCache::response(
        &identity,
        binding,
        200,
        b"User-agent: *\nAllow: /\n",
        millis().unwrap(),
    ));
    let now = format_time(SystemTime::now()).unwrap();
    let url = "https://garden.example/docs/start";
    assert!(admit(&fixture.policy, &request("notes", url, &now), &controls).is_ok());
    let preview = super::command::preview(&fixture.policy, &controls, SystemTime::now()).unwrap();
    assert!(preview.completed.is_empty());
    assert_eq!(preview.decisions.len(), 1);
    assert_eq!(preview.decisions.first().unwrap().reason, "allowed");
    assert!(
        admit(
            &fixture.policy,
            &request("notes", url, "2026-09-30T00:00:00+02:00"),
            &controls
        )
        .is_err()
    );
    fixture.finish();
}
