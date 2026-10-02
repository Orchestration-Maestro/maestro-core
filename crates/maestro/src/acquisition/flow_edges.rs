//! Completion, reuse, depth, bindings and resource boundaries of the public MVP.
use super::{
    bindings,
    command::{resolve, sync},
    controls::{Controls, Runtime},
    flow_tests::{Dns, Grants, Host, Leaf, Site, fixture_with},
    resources,
};
use maestro_acquisition::{
    Principal,
    lifecycle::{
        full::Mode,
        resources::{ResourceControls, ResourceSnapshot, Resources},
    },
    policy::limits::Limits,
    transport::{
        budget::{Pending, Usage},
        pacing::OriginLedger,
    },
};
use maestro_kernel::{
    acquisition::{Captures, Enumeration, Frontier, Handle, Partitions, Status},
    artifact::Digest,
    scope::Right,
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use serde_json::json;
use std::{
    env, fs,
    sync::Arc,
    time::{Instant, SystemTime},
};
use tokio::runtime::Builder;

pub(super) use super::flow_fixture::{Fixture, clean};

#[test]
fn n14_clean_site_completes_and_immediate_repeat_reuses_without_fetch() {
    let fixture = Fixture::new(clean);
    let first = fixture.sync();
    assert_eq!(first.status, Status::Complete, "{first:?}");
    assert_eq!(first.completed.len(), 3);
    assert!(first.pending.is_empty());
    let scope = "workspace/default/collection/garden".parse().unwrap();
    let accepted_links = Partitions::partition_page(&fixture.db, &scope, "notes", None, 1000)
        .unwrap()
        .into_iter()
        .map(|id| {
            Partitions::partition(&fixture.db, &scope, id)
                .unwrap()
                .unwrap()
        })
        .filter(|state| {
            state.batches.first().unwrap().partition.kind == Enumeration::Links
                && state.accepted.is_some()
        })
        .count();
    assert_eq!(accepted_links, 3);
    let requests = fixture.site.requests.lock().unwrap().len();
    let second = fixture.sync();
    assert_ne!(first.run, second.run);
    assert_ne!(first.receipt, second.receipt);
    assert_eq!(second.status, Status::Complete, "{second:?}");
    assert_eq!(fixture.site.requests.lock().unwrap().len(), requests);
    assert!(
        second
            .completed
            .iter()
            .all(|entry| entry.reason == "captured_earlier_not_revalidated"
                && entry.observed_ms.is_some())
    );
    let bytes = serde_json::to_vec(&second).unwrap();
    assert!(!String::from_utf8(bytes).unwrap().contains("https://"));
    fixture.finish();
}

#[test]
fn n14_capture_lookup_verifies_scope_and_corrupt_link() {
    let fixture = Fixture::new(clean);
    fixture.sync();
    let item = Frontier::page(&fixture.db, &fixture.scopes, "notes", None, 1000)
        .unwrap()
        .remove(0);
    let handle = fixture
        .db
        .capture_for(&fixture.scope, &item)
        .unwrap()
        .unwrap();
    let another = "workspace/default/collection/another".parse().unwrap();
    assert_eq!(fixture.db.capture_for(&another, &item).unwrap(), None);
    let connection = rusqlite::Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    // Simulate damaged durable linkage, not an ordinary writable capture.
    connection
        .execute_batch(
            "PRAGMA foreign_keys=OFF; DROP TRIGGER acquisition_capture_links_never_change",
        )
        .unwrap();
    connection
        .execute(
            "UPDATE acquisition_capture_links SET envelope=?1 WHERE item=?2",
            rusqlite::params![Handle::new().to_string(), item.id.to_string()],
        )
        .unwrap();
    assert_eq!(fixture.db.capture_for(&fixture.scope, &item).unwrap(), None);
    connection
        .execute(
            "UPDATE acquisition_capture_links SET envelope=?1 WHERE item=?2",
            rusqlite::params![handle.to_string(), item.id.to_string()],
        )
        .unwrap();
    drop(connection);
    fixture.finish();
}

#[test]
fn n14_declared_depth_is_discarded_but_run_depth_is_pending() {
    let mut declared = Fixture::new(clean);
    declared.site.leaf = Leaf::Linked;
    let report = declared.sync();
    assert_eq!(report.status, Status::Complete, "{report:?}");
    assert!(
        report
            .discarded
            .iter()
            .any(|entry| entry.reason == "beyond_declared_depth")
    );
    declared.finish();
    let mut capped = Fixture::new(|value| {
        clean(value);
        value["sources"][0]["discovery"][0]["depth"] = json!(2);
    });
    capped.site.leaf = Leaf::Linked;
    // Link scope is two levels; operational depth stays one. The leaf checkpoint must hold.
    let report = capped.sync();
    assert_eq!(report.status, Status::Partial, "{report:?}");
    assert!(
        report
            .pending
            .iter()
            .any(|entry| entry.reason == "run_depth_limit")
    );
    capped.finish();
}

/// Synthetic measurement seam proves the existing N11 floor, not a disk-number binding.
#[derive(Debug)]
struct Measured {
    limits: Limits,
    free: u64,
}
impl ResourceControls for Measured {
    fn snapshot(&self) -> Result<ResourceSnapshot, Pending> {
        Ok(ResourceSnapshot {
            aggregate: self.limits.clone(),
            per_run: self.limits.clone(),
            free_disk_bytes: self.free,
            free_gpu_bytes: 0,
            interactive_pending: false,
        })
    }
}
#[test]
fn n14_oa3_tightens_policy_and_refuses_disk_floor_and_unsupported_host() {
    let fixture = Fixture::new(clean);
    let mut policy = fixture.policy.policy().aggregate_limits.clone();
    policy.cpu_millicores = 20_000.try_into().unwrap();
    policy.memory_bytes = 50_000_000_000.try_into().unwrap();
    policy.staging_bytes = 50_000_000_000.try_into().unwrap();
    policy.origin_concurrency = 20.try_into().unwrap();
    policy.source_runs = 20.try_into().unwrap();
    let run = resources::bounds(&policy, false).unwrap();
    assert_eq!(run.cpu_millicores.get(), 2000);
    assert_eq!(run.memory_bytes.get(), 4_294_967_296);
    assert_eq!(run.staging_bytes.get(), 10_737_418_240);
    assert_eq!(run.origin_concurrency.get(), 1);
    assert_eq!(run.source_runs.get(), 4);
    assert_eq!(run.free_reserve_bytes.get(), 32_212_254_720);
    let narrower = resources::bounds(&fixture.policy.policy().aggregate_limits, false).unwrap();
    assert_eq!(
        narrower.memory_bytes,
        fixture.policy.policy().aggregate_limits.memory_bytes
    );
    let resources = Resources::new(Arc::new(Measured {
        limits: run.clone(),
        free: run.free_reserve_bytes.get() - 1,
    }));
    assert_eq!(
        resources.reserve(&[run], Usage::default()).unwrap_err(),
        Pending::DiskReserve
    );
    assert!(resources::supported("linux").is_ok());
    for platform in ["windows", "macos"] {
        assert!(resources::supported(platform).is_err());
        assert!(
            resources::supported(platform)
                .unwrap_err()
                .to_string()
                .contains("live resource control unsupported on this host")
        );
    }
    fixture.finish();
}

#[test]
fn n14_direct_bindings_refuse_missing_unknown_and_unbound_resources() {
    let fixture = Fixture::new(clean);
    let principal = Principal {
        id: "reader",
        platform: env::consts::OS,
        scopes: &fixture.scopes,
    };
    let (collection, files) = fixture_with(clean);
    let manifest = fixture.root.join("manifest.json");
    fs::write(&manifest, serde_json::to_vec(&collection).unwrap()).unwrap();
    let mut resources = Vec::new();
    for (id, resource) in &files.0 {
        fs::write(fixture.root.join(format!("{id}.json")), &resource.bytes).unwrap();
        resources.push(json!({
            "id":id,"path":format!("{id}.json"),
            "admission":{
                "digest":resource.reference.digest,"platform":env::consts::OS,
                "capabilities":resource.admission.capabilities,"status":"reviewed",
                "references":resource.admission.references
            }
        }));
    }
    let path = fixture.root.join("bindings.json");
    let valid = json!({"schema":"maestro-acquisition-bindings/1","resources":resources});
    fs::write(&path, serde_json::to_vec(&valid).unwrap()).unwrap();
    assert_eq!(
        bindings::load(&manifest, &path, &principal)
            .unwrap()
            .0
            .reference()
            .digest,
        fixture.policy.reference().digest
    );
    for change in [
        "missing",
        "unknown",
        "unbound",
        "duplicate",
        "changed",
        "held",
        "proposed",
        "revoked",
        "badplatform",
    ] {
        let mut wire = valid.clone();
        let entries = wire["resources"].as_array_mut().unwrap();
        match change {
            "missing" => {
                entries.retain(|entry| entry["id"] != "decisions");
            }
            "unbound" => {
                entries
                    .iter_mut()
                    .find(|entry| entry["id"] == "policy")
                    .unwrap()["path"] = json!("absent.json");
            }
            "duplicate" => {
                entries.push(entries.first().unwrap().clone());
            }
            "changed" => {
                entries.first_mut().unwrap()["admission"]["digest"] =
                    json!(Digest::of(b"wrong original bytes"));
            }
            "held" | "proposed" | "revoked" => {
                entries.first_mut().unwrap()["admission"]["status"] = json!(change);
            }
            "badplatform" => {
                entries.first_mut().unwrap()["admission"]["platform"] = json!("unsupported");
            }
            _ => {
                let mut entry = entries.first().unwrap().clone();
                entry["id"] = json!("unknown");
                entries.push(entry);
            }
        }
        fs::write(&path, serde_json::to_vec(&wire).unwrap()).unwrap();
        assert!(
            bindings::load(&manifest, &path, &principal).is_err(),
            "{change}"
        );
        assert!(fixture.site.requests.lock().unwrap().is_empty());
        assert!(
            Frontier::page(&fixture.db, &fixture.scopes, "notes", None, 1000)
                .unwrap()
                .is_empty()
        );
    }
    fixture.finish();
}

#[test]
fn n14_non_success_response_stays_pending_without_capture() {
    let mut fixture = Fixture::new(clean);
    fixture.site.failure = true;
    let report = fixture.sync();
    assert_eq!(report.status, Status::Partial);
    assert!(report.completed.is_empty());
    assert!(
        report
            .pending
            .iter()
            .any(|entry| entry.reason == "transport")
    );
    assert!(
        Frontier::page(&fixture.db, &fixture.scopes, "notes", None, 1000)
            .unwrap()
            .iter()
            .all(|item| item.capture.is_none())
    );
    fixture.finish();
}

#[test]
fn n14_public_scope_and_unsupported_discovery_refuse_before_starts() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scopes = db.visible("reader").unwrap();
    let principal = Principal {
        id: "reader",
        platform: env::consts::OS,
        scopes: &scopes,
    };
    let (collection, files) = fixture_with(clean);
    db.grant(
        "reader",
        &"workspace/default/collection/garden".parse().unwrap(),
        Right::Read,
        "owner",
    )
    .unwrap();
    let authorized = db.visible("reader").unwrap();
    let checked_principal = Principal {
        id: "reader",
        platform: env::consts::OS,
        scopes: &authorized,
    };
    // Direct resolution checks source capability without any transport or credential port.
    let (unsupported, resources) = fixture_with(|policy| {
        policy["sources"][0]["discovery"] =
            json!([{"kind":"sitemap", "url":"https://garden.example/docs/index.xml"}]);
    });
    assert!(resolve(&resources, &unsupported, &checked_principal).is_err());
    // A checked public declaration still needs the caller's current collection scope.
    let checked = resolve(&files, &collection, &checked_principal).unwrap();
    let site = Site::default();
    let grants = Grants::default();
    let dns = Dns::default();
    let controls = Controls::new(
        &checked,
        &grants,
        "reader",
        "workspace/default/collection/garden",
    );
    let resources = Resources::new(Arc::new(Host(checked.policy().aggregate_limits.clone())));
    let pacing = OriginLedger::new(0);
    let runtime = Runtime {
        authority: &grants,
        resolver: &dns,
        transport: &site,
        pacing: &pacing,
        controls: &controls,
        epoch: Instant::now(),
        run_now: SystemTime::now(),
        clock: &SystemTime::now,
        mode: Mode::Incremental,
        kernel_principal: "reader",
        frontier_page_size: 2,
        collection: Digest::of(&serde_json::to_vec(&collection).unwrap()),
    };
    assert!(
        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(sync(&db, &checked, &principal, &runtime, &resources))
            .is_err()
    );
    assert!(site.requests.lock().unwrap().is_empty());
    assert!(
        Frontier::page(&db, &scopes, "notes", None, 1000)
            .unwrap()
            .is_empty()
    );
    drop(db);
    fs::remove_dir_all(root).unwrap();
}
