//! Synthetic public site through the production composition, without sockets or credentials.
use super::{
    command::{preview, resolve, sync},
    controls::{Controls, Runtime},
    inspect::inspect,
};
use maestro_acquisition::{
    Admission, AdmissionStatus, CheckedPolicy, ImmutableResource, PolicySource, Principal, Ref,
    Refusal, ResourceSource,
    lifecycle::{
        full::Mode,
        resources::{ResourceControls, ResourceSnapshot, Resources},
    },
    policy::{
        authority::{Authority, AuthorityRefusal, Operation, Permit, Target},
        limits::Limits,
    },
    transport::{budget::Pending, connect::Resolver, pacing::OriginLedger},
    validate,
};
use maestro_kernel::{
    acquisition::{Frontier, Receipts as _, Status},
    artifact::Digest,
    scope::Right,
    store::Database,
};
use maestro_knowledge::collection::Declaration;
use maestro_test_scratch::scratch_directory;
use serde_json::{Value, json};
use std::{
    cell::Cell,
    collections::BTreeMap,
    env, fs,
    sync::Arc,
    time::{Instant, SystemTime},
};
use tokio::runtime::Builder;

/// Independently authored immutable fixture resources.
#[derive(Debug)]
pub(super) struct Files(pub(super) BTreeMap<String, ImmutableResource>);
impl ResourceSource for Files {
    fn read(&self, reference: &Ref, _: &Principal<'_>) -> Result<ImmutableResource, Refusal> {
        self.0.get(&reference.id).cloned().ok_or(Refusal::Missing)
    }
}
impl PolicySource for Files {
    fn resolve(
        &self,
        collection: &Declaration,
        principal: &Principal<'_>,
    ) -> Result<CheckedPolicy, Refusal> {
        validate(self, collection, principal)
    }
}
/// Bind authored references by identity, not production serialization expectations.
fn bind(value: &mut Value, files: &Files) {
    match value {
        Value::Object(fields) if fields.len() == 2 && fields.contains_key("digest") => {
            let id = fields.get("id").unwrap().as_str().unwrap();
            let digest = files.0.get(id).unwrap().reference.digest.clone();
            fields.insert("digest".into(), json!(digest));
        }
        Value::Object(fields) => {
            for value in fields.values_mut() {
                bind(value, files);
            }
        }
        Value::Array(values) => {
            for value in values {
                bind(value, files);
            }
        }
        _ => {}
    }
}
/// Trusted test admission, never a production grant or flag.
fn put(files: &mut Files, id: &str, value: &Value) {
    let bytes = serde_json::to_vec(value).unwrap();
    let digest = Digest::of(&bytes);
    files.0.insert(
        id.into(),
        ImmutableResource {
            reference: Ref {
                id: id.into(),
                digest: digest.clone(),
            },
            bytes,
            admission: Admission {
                digest,
                platform: env::consts::OS.into(),
                capabilities: vec!["http".into(), "fetch".into()],
                status: AdmissionStatus::Reviewed,
                references: vec![],
            },
        },
    );
}
/// A complete public policy with actual denials, bounded discovery and no S3/S4.
pub(super) fn fixture() -> (Declaration, Files) {
    fixture_with(|_| {})
}
/// Edit checked source data before independently binding its original digest.
pub(super) fn fixture_with(mut edit: impl FnMut(&mut Value)) -> (Declaration, Files) {
    fixture_values(|id, value| {
        if id == "policy" {
            edit(value);
        }
    })
}
/// Edit metadata/resources before the same topological original-digest binding.
pub(super) fn fixture_values(mut edit: impl FnMut(&str, &mut Value)) -> (Declaration, Files) {
    let mut files = Files(BTreeMap::new());
    for id in [
        "owner",
        "evidence",
        "qualification",
        "matrix",
        "thresholds",
        "baseline",
        "retention",
        "adapter",
        "markdown",
        "extraction",
    ] {
        put(&mut files, id, &json!({"synthetic":id}));
    }
    let markdown = files.0.get("markdown").unwrap().reference.clone();
    files
        .0
        .get_mut("extraction")
        .unwrap()
        .admission
        .references
        .push(markdown);
    for (id, text) in [
        (
            "addresses",
            include_str!("../../../maestro-acquisition/tests/fixtures/address-table.json"),
        ),
        (
            "decisions",
            include_str!("../../../maestro-acquisition/tests/fixtures/decisions.json"),
        ),
        (
            "http",
            include_str!("../../../maestro-acquisition/tests/fixtures/http.json"),
        ),
        (
            "policy",
            include_str!("../../../maestro-acquisition/tests/fixtures/policy.json"),
        ),
    ] {
        let mut value: Value = serde_json::from_str(text).unwrap();
        if id == "decisions" {
            value["entries"][0]["selector"]["path_prefix"] = json!("/docs/private");
            value["entries"][0]["action"] = json!("deny_fetch");
        }
        if id == "policy" {
            let limits = value.pointer_mut("/sources/0/limits").unwrap();
            for key in [
                "requests",
                "pages",
                "partitions",
                "wire_bytes",
                "dom_bytes",
                "asset_bytes",
                "staging_bytes",
                "memory_bytes",
                "cpu_millicores",
            ] {
                limits
                    .as_object_mut()
                    .unwrap()
                    .insert(key.into(), json!(10_000_000));
            }
            limits["pages"] = json!(50);
            limits["partitions"] = json!(50);
            limits["redirects"] = json!(5);
            limits["max_backoff_ms"] = json!(60000);
            value["aggregate_limits"] = limits.clone();
            value["sources"][0]["robots"]["cache_ttl_ms"] = json!(86_400_000);
            value["sources"][0]["seeds"] = json!([
                "https://garden.example/docs/start",
                "https://garden.example/docs/private"
            ]);
        }
        edit(id, &mut value);
        bind(&mut value, &files);
        put(&mut files, id, &value);
    }
    let mut collection: Value = serde_json::from_str(include_str!(
        "../../../maestro-acquisition/tests/fixtures/collection.json"
    ))
    .unwrap();
    edit("collection", &mut collection);
    bind(&mut collection, &files);
    (serde_json::from_value(collection).unwrap(), files)
}
/// Current exact-target synthetic authority; never credentials.
#[derive(Debug, Default)]
pub(super) struct Grants(Cell<usize>);
impl Authority for Grants {
    fn decide(
        &self,
        _: &str,
        _: Operation,
        _: &Target,
        _: SystemTime,
    ) -> Result<Permit, AuthorityRefusal> {
        self.0.set(self.0.get() + 1);
        Ok(Permit {
            grant_id: "synthetic".into(),
        })
    }
}
/// Public DNS fixture; transport cannot change the admitted socket.
#[derive(Debug, Default)]
pub(super) struct Dns(Cell<usize>);
impl Resolver for Dns {
    fn resolve(&self, _: &str) -> Result<Vec<String>, Refusal> {
        self.0.set(self.0.get() + 1);
        Ok(vec!["8.8.8.8".into()])
    }
}
pub(super) use super::flow_site::{Leaf, Revision, Site};
/// Explicit synthetic measurements, not a CLI-selected bypass.
#[derive(Debug)]
pub(super) struct Host(pub(super) Limits);
impl ResourceControls for Host {
    fn snapshot(&self) -> Result<ResourceSnapshot, Pending> {
        Ok(ResourceSnapshot {
            aggregate: self.0.clone(),
            per_run: self.0.clone(),
            free_disk_bytes: 1_000_000_000_000,
            free_gpu_bytes: 0,
            interactive_pending: false,
        })
    }
}

#[test]
fn n14_public_preview_sync_and_durable_inspect_share_one_flow() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scope = "workspace/default/collection/garden".parse().unwrap();
    db.grant("reader", &scope, Right::Read, "owner").unwrap();
    let scopes = db.visible("reader").unwrap();
    let principal = Principal {
        id: "reader",
        platform: env::consts::OS,
        scopes: &scopes,
    };
    let (collection, files) = fixture();
    let policy = resolve(&files, &collection, &principal).unwrap();
    let grants = Grants::default();
    let dns = Dns::default();
    let site = Site::default();
    let controls = Controls::new(&policy, &grants, "reader", scope.as_str());
    let preview = preview(&policy, &controls, SystemTime::now()).unwrap();
    assert_eq!(preview.discarded.len(), 1);
    assert_eq!(preview.pending.len(), 1); // robots not fetched, no invented allow.
    assert_eq!(dns.0.get(), 0);
    assert!(site.requests.lock().unwrap().is_empty());
    assert!(grants.0.get() > 0);
    assert!(
        Frontier::page(&db, &scopes, "notes", None, 1000)
            .unwrap()
            .is_empty()
    );
    let resources = Resources::new(Arc::new(Host(policy.policy().aggregate_limits.clone())));
    let runtime = Runtime {
        authority: &grants,
        resolver: &dns,
        transport: &site,
        pacing: &OriginLedger::new(0),
        controls: &controls,
        epoch: Instant::now(),
        run_now: SystemTime::now(),
        clock: &SystemTime::now,
        mode: Mode::Incremental,
        kernel_principal: "reader",
        frontier_page_size: 2,
        collection: Digest::of(&serde_json::to_vec(&collection).unwrap()),
    };
    let report = Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(sync(&db, &policy, &principal, &runtime, &resources))
        .unwrap();
    let requests = check_partial(&report, &site);
    assert!(
        db.inspect("reader", report.receipt.unwrap())
            .unwrap()
            .is_some()
    );
    drop(controls);
    drop(db);
    let reopened = Database::open_in(&root).unwrap();
    assert_eq!(
        inspect(&reopened, "reader", report.receipt.unwrap()).unwrap(),
        report
    );
    assert_eq!(site.requests.lock().unwrap().len(), requests.len());
    assert!(inspect(&reopened, "denied", report.receipt.unwrap()).is_err());
    drop(reopened);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn n14_invalid_policy_has_zero_effects() {
    let root = scratch_directory().unwrap();
    let db = Database::open_in(&root).unwrap();
    let scopes = db.visible("reader").unwrap();
    let principal = Principal {
        id: "reader",
        platform: env::consts::OS,
        scopes: &scopes,
    };
    let (collection, mut files) = fixture();
    files.0.remove("decisions");
    assert!(resolve(&files, &collection, &principal).is_err());
    assert!(
        Frontier::page(&db, &scopes, "notes", None, 1000)
            .unwrap()
            .is_empty()
    );
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

/// All partial references retain an explicit reason and denied destinations see no effects.
fn check_partial(report: &super::output::Report, site: &Site) -> Vec<String> {
    assert_eq!(report.status, Status::Partial);
    assert_eq!(
        report.completed.len(),
        4,
        "{report:?}; requests={:?}",
        site.requests.lock().unwrap()
    );
    assert!(
        report
            .pending
            .iter()
            .any(|item| item.reason == "robots_denied")
    );
    assert!(
        report
            .pending
            .iter()
            .any(|item| item.reason == "unresolved_identity")
    );
    assert!(
        report
            .discarded
            .iter()
            .any(|item| item.reason == "policy_denial")
    );
    assert!(
        report
            .discarded
            .iter()
            .any(|item| item.reason == "non_fetch_scheme")
    );
    assert!(
        report
            .discarded
            .iter()
            .any(|item| item.reason == "beyond_declared_depth")
    );
    let requests = site.requests.lock().unwrap().clone();
    for denied in ["/docs/private", "/docs/robot", "/docs/beyond"] {
        assert!(!requests.iter().any(|path| path == denied));
    }
    assert!(requests.iter().any(|path| path == "/docs/file.pdf"));
    assert!(requests.iter().any(|path| path == "/docs/final"));
    requests
}
