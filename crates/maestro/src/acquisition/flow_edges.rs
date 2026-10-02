//! Completion, reuse, depth, bindings and resource boundaries of the public MVP.
use super::{
    bindings,
    command::{resolve, sync},
    controls::{Controls, Runtime},
    flow_tests::{Dns, Grants, Host, Leaf, Site, fixture_with},
    output::Report,
    resources,
};
use crate::failure::Failure;
use maestro_acquisition::{
    CheckedPolicy, Principal,
    lifecycle::resources::{ResourceControls, ResourceSnapshot, Resources},
    policy::limits::Limits,
    transport::{
        budget::{Pending, Usage},
        pacing::OriginLedger,
    },
};
use maestro_kernel::{
    acquisition::{Captures, Frontier, Handle, Status},
    artifact::Digest,
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use maestro_test_scratch::scratch_directory;
use serde_json::{Value, json};
use std::{env, fs, path::PathBuf, sync::Arc, time::Instant};
use tokio::runtime::Builder;

/// A real kernel and replaceable synthetic site; no source host or catalog exists.
pub(super) struct Fixture {
    /// Real scoped storage.
    pub(super) db: Database,
    /// Owned test directory, removed after closing the database.
    root: PathBuf,
    /// Current authorized collection scope.
    scope: Scope,
    /// Current grants from the real kernel.
    scopes: ScopeSet,
    /// Validated immutable executable policy.
    pub(super) policy: CheckedPolicy,
    /// The composition root freezes one OS-to-kernel mapping, without setters.
    os_principal: String,
    /// The kernel authorizer of this OS caller.
    kernel_principal: String,
    /// Independently bound collection bytes.
    pub(super) collection: Digest,
    /// Explicit synthetic public wire adapter.
    pub(super) site: Site,
}
impl Fixture {
    /// Each case changes source data only, never production callers.
    pub(super) fn new(edit: impl FnMut(&mut Value)) -> Self {
        Self::mapped(edit, "reader", "reader")
    }
    /// Inject the same explicit mapping as the local composition root.
    pub(super) fn mapped(
        edit: impl FnMut(&mut Value),
        os_principal: &str,
        kernel_principal: &str,
    ) -> Self {
        let root = scratch_directory().unwrap();
        let db = Database::open_in(&root).unwrap();
        let scope = "workspace/default/collection/garden".parse().unwrap();
        db.grant(kernel_principal, &scope, Right::Read, "owner")
            .unwrap();
        let scopes = db.visible(kernel_principal).unwrap();
        let principal = Principal {
            id: os_principal,
            platform: env::consts::OS,
            scopes: &scopes,
        };
        let (collection, files) = fixture_with(edit);
        let policy = resolve(&files, &collection, &principal).unwrap();
        Self {
            root,
            db,
            scope,
            scopes,
            policy,
            os_principal: os_principal.into(),
            kernel_principal: kernel_principal.into(),
            collection: Digest::of(&serde_json::to_vec(&collection).unwrap()),
            site: Site {
                clean: true,
                ..Site::default()
            },
        }
    }
    /// Invoke unchanged production composition with synthetic ports.
    pub(super) fn sync(&self) -> Report {
        self.try_sync().unwrap()
    }
    /// The same production API retains typed refusal evidence.
    pub(super) fn try_sync(&self) -> Result<Report, Failure> {
        self.try_sync_page_size(2)
    }
    /// Exercise production cursor size without changing the ordinary regression fixtures.
    pub(super) fn try_sync_page_size(&self, page_size: u16) -> Result<Report, Failure> {
        let scopes = self.db.visible(&self.kernel_principal).unwrap();
        let principal = Principal {
            id: &self.os_principal,
            platform: env::consts::OS,
            scopes: &scopes,
        };
        let grants = Grants::default();
        let dns = Dns::default();
        let controls = Controls::new(
            &self.policy,
            &grants,
            &self.os_principal,
            self.scope.as_str(),
        );
        let resources = Resources::new(Arc::new(Host(
            self.policy.policy().aggregate_limits.clone(),
        )));
        let pacing = OriginLedger::new(0);
        let runtime = Runtime {
            authority: &grants,
            resolver: &dns,
            transport: &self.site,
            pacing: &pacing,
            controls: &controls,
            epoch: Instant::now(),
            collection: self.collection.clone(),
            kernel_principal: &self.kernel_principal,
            frontier_page_size: page_size,
        };
        Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(sync(
                &self.db,
                &self.policy,
                &principal,
                &runtime,
                &resources,
            ))
    }
    /// Close SQLite before removing files on Windows too.
    pub(super) fn finish(self) {
        drop(self.db);
        fs::remove_dir_all(self.root).unwrap();
    }
}
/// Clean capture scope, distinct child bytes and bounded empty leaf inventories.
pub(super) fn clean(value: &mut Value) {
    value["sources"][0]["seeds"] = json!(["https://garden.example/docs/start"]);
}

#[test]
fn n14_clean_site_completes_and_immediate_repeat_reuses_without_fetch() {
    let fixture = Fixture::new(clean);
    let first = fixture.sync();
    assert_eq!(first.status, Status::Complete, "{first:?}");
    assert_eq!(first.completed.len(), 3);
    assert!(first.pending.is_empty());
    let connection = rusqlite::Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
    assert_eq!(
        connection
            .query_row(
                "SELECT count(*) FROM acquisition_partition_snapshots",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        3
    );
    drop(connection);
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
