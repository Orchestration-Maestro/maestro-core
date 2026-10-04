//! N07 URL and denial contracts use only synthetic destinations.
#![expect(clippy::indexing_slicing, reason = "authored fixture positions")]
use super::support;
use maestro_acquisition::policy::{
    decision::{AdmissionControls, Disposition, ItemAttributes, Request, RequestKind, admit},
    identity::{DisplayLink, FetchIdentity, SignedTransferUrl},
    source::{QueryOrder, RepeatedQueries, Source},
};
use maestro_acquisition::{CheckedPolicy, Refusal, parse_policy, validate};
use maestro_kernel::artifact::Store;
use maestro_knowledge::collection::Declaration;
use serde_json::{Value, json};
use std::{cell::RefCell, fs};

/// Current controls record only their stage, never URL content.
#[derive(Debug, Default)]
pub(super) struct Controls {
    /// Inject a content-free refusal from this stage.
    deny: &'static str,
    /// Observed stage order.
    stages: RefCell<Vec<&'static str>>,
}
impl Controls {
    /// Refusal is observed by the real shared admission entry point.
    fn check(&self, stage: &'static str) -> Result<(), Refusal> {
        self.stages.borrow_mut().push(stage);
        if self.deny == stage {
            Err(Refusal::Access)
        } else {
            Ok(())
        }
    }
}
impl AdmissionControls for Controls {
    fn caller(&self, _source: &Source, _request: &Request<'_>) -> Result<(), Refusal> {
        self.check("caller")
    }
    fn network(&self, _identity: &FetchIdentity) -> Result<(), Refusal> {
        self.check("network")
    }
    fn robots(&self, _identity: &FetchIdentity) -> Result<(), Refusal> {
        self.check("robots")
    }
}

/// N07 adds a real deny boundary without changing N03's fixture bytes.
pub(super) fn fixture() -> (Value, support::Catalog) {
    let (mut collection, mut catalog) = support::fixture();
    let mut decisions = support::value(&catalog, "decisions");
    decisions["entries"][0]["action"] = "deny_fetch".into();
    decisions["entries"][0]["selector"]["path_prefix"] = "/docs/private".into();
    support::put(&mut catalog, "decisions", &decisions);
    support::rebind(&mut collection, &mut catalog);
    (collection, catalog)
}
/// Strict source authored in the already checked N03 fixture.
fn source() -> Source {
    let (_, catalog) = fixture();
    parse_policy(&support::value(&catalog, "policy").to_string())
        .unwrap()
        .sources
        .remove(0)
}
/// Resolve the full immutable closure with current kernel grants.
pub(super) fn checked(
    collection: Value,
    catalog: &support::Catalog,
) -> Result<CheckedPolicy, Refusal> {
    let declaration: Declaration = serde_json::from_value(collection).unwrap();
    validate(
        catalog,
        &declaration,
        &support::principal(&support::scopes()),
    )
}
/// Every request kind uses the same controls, including explicit cache bypass.
pub(super) fn request(url: &str, kind: RequestKind) -> Request<'_> {
    Request {
        source_id: "notes",
        url,
        kind,
        attributes: ItemAttributes::default(),
        cache_bypass: true,
        now: "2026-09-30T20:00:00Z",
    }
}

/// Parsing must inspect the supplied path, not a parser-normalized replacement.
#[test]
fn n07_rejects_normalized_paths_and_encoded_controls() {
    let (_, catalog) = fixture();
    for url in [
        "https://garden.example/docs/x/../start",
        "https://garden.example/docs/./start",
        "https://garden.example/docs/%2e/start",
        "https://garden.example/docs/%01start",
        "https://garden.example/docs/%7fstart",
        "https://garden.example/docs/%zz",
        "https://garden.example/docs/%",
        "https://garden.example/docs/start#%6",
        "https://garden.example/docs/start#%0a",
        "https://garden.example/docs/start\u{0085}",
    ] {
        let mut policy = support::value(&catalog, "policy");
        policy["sources"][0]["seeds"][0] = url.into();
        assert!(
            parse_policy(&policy.to_string()).is_err(),
            "admitted {url:?}"
        );
    }
}

/// Allowed neighbours exercise the same identity parser as refused boundaries.
#[test]
fn n07_exact_origins_paths_and_safe_references() {
    let source = source();
    for path in ["/docs", "/docs/", "/docs/start", "/docs/start#section"] {
        assert!(FetchIdentity::parse(&source, &format!("https://garden.example{path}")).is_ok());
    }
    for url in [
        "https://garden.example/docs-neighbour",
        "https://garden.example/doc",
        "https://garden.example:444/docs",
        "http://garden.example/docs",
        "https://other.example/docs",
        "https://garden.example./docs",
        "https://Garden.example/docs",
        "https://garden.example@other.example/docs",
        "https://user@garden.example/docs",
        "https://@garden.example/docs",
        "https://garden.example/docs%2fprivate",
        "https://garden.example/docs/%5cprivate",
        "https://garden.example/docs/%252fprivate",
        "https://garden.example/docs\n/start",
        "https://garden.example/docs//start",
        "https://garden.example/docs/%ff",
    ] {
        assert!(
            FetchIdentity::parse(&source, url).is_err(),
            "admitted {url:?}"
        );
    }
    let plain = FetchIdentity::parse(&source, "https://garden.example/docs#section").unwrap();
    let explicit = FetchIdentity::parse(&source, "https://garden.example:443/docs").unwrap();
    assert_eq!(plain, explicit);
    assert_eq!(plain.as_str(), "https://garden.example/docs");
    assert_eq!(plain.source_id(), "notes");
    assert_eq!(plain.origin_id(), "content");
    assert_eq!(plain.version().get(), 1);
    let link = DisplayLink::parse("https://garden.example/docs#section").unwrap();
    assert_eq!(link.as_str(), "https://garden.example/docs#section");
    let transfer =
        SignedTransferUrl::parse("https://garden.example/docs?signature=canary").unwrap();
    assert!(FetchIdentity::parse(&source, transfer.as_str()).is_err());
    assert!(!format!("{transfer:?}").contains("canary"));
    assert!(!format!("{link:?}").contains("section"));
}

/// Meaningful raw query bytes survive; sorting never reorders repeated values.
#[test]
fn n07_query_semantics_are_explicit_and_lossless() {
    let mut source = source();
    source.identity.meaningful_queries = vec!["v".into(), "lang".into()];
    source.identity.ignored_tracking_queries = vec!["track".into()];
    let parse = |source: &Source, suffix: &str| {
        FetchIdentity::parse(source, &format!("https://garden.example/docs{suffix}"))
    };
    let identity = parse(&source, "?v=a+b&lang=%2B&track=secret#part").unwrap();
    assert_eq!(
        identity.as_str(),
        "https://garden.example/docs?v=a+b&lang=%2B"
    );
    assert_ne!(identity, parse(&source, "?lang=%2B&v=a+b").unwrap());
    assert_ne!(
        parse(&source, "?v").unwrap(),
        parse(&source, "?v=").unwrap()
    );
    for suffix in [
        "?unknown=1",
        "?v=1&v=2",
        "?track=1&track=2",
        "?v=%00",
        "?v=%ff",
        "?v=%zz",
        "?v=1&&lang=en",
        "?",
    ] {
        assert!(parse(&source, suffix).is_err(), "admitted {suffix}");
    }
    source.identity.repeated_queries = RepeatedQueries::Preserve;
    source.identity.query_order = QueryOrder::Sort;
    assert_eq!(
        parse(&source, "?v=2&lang=en&v=1").unwrap().as_str(),
        "https://garden.example/docs?lang=en&v=2&v=1"
    );
    assert_ne!(
        parse(&source, "?v=2&v=1").unwrap(),
        parse(&source, "?v=1&v=2").unwrap()
    );
    source.identity.ignored_tracking_queries.push("v".into());
    assert!(parse(&source, "?v=1").is_err());
}

/// All bypass attempts encounter current authority, network, robots and denials.
#[test]
fn n07_denials_defeat_every_request_kind_promotion_and_cache_bypass() {
    let (collection, catalog) = fixture();
    let policy = checked(collection, &catalog).unwrap();
    for kind in [
        RequestKind::Seed,
        RequestKind::Redirect,
        RequestKind::Subresource,
        RequestKind::Resume,
        RequestKind::Retry,
    ] {
        for stage in ["caller", "network", "robots"] {
            let controls = Controls {
                deny: stage,
                ..Controls::default()
            };
            let mut requests = 0;
            if admit(
                &policy,
                &request("https://garden.example/docs/start", kind),
                &controls,
            )
            .is_ok()
            {
                requests += 1;
            }
            assert_eq!(requests, 0, "{kind:?} bypassed {stage}");
            let expected = match stage {
                "caller" => vec!["caller"],
                "network" => vec!["caller", "network"],
                _ => vec!["caller", "network", "robots"],
            };
            assert_eq!(*controls.stages.borrow(), expected);
        }
        let controls = Controls::default();
        assert!(
            admit(
                &policy,
                &request("https://garden.example/docs-neighbour", kind),
                &controls
            )
            .is_err()
        );
        assert_eq!(*controls.stages.borrow(), vec!["caller"]);
        let denied = Controls::default();
        assert!(
            admit(
                &policy,
                &request("https://garden.example/docs/private", kind),
                &denied
            )
            .is_err()
        );
        assert_eq!(*denied.stages.borrow(), vec!["caller"]);
    }
    let allowed = admit(
        &policy,
        &request(
            "https://garden.example/docs/private-neighbour",
            RequestKind::Seed,
        ),
        &Controls::default(),
    )
    .unwrap();
    assert_eq!(allowed.disposition(), Disposition::Knowledge);
    assert!(allowed.cache_bypass());
    let denied_first = Controls {
        deny: "caller",
        ..Controls::default()
    };
    assert_eq!(
        admit(
            &policy,
            &request("not a URL", RequestKind::Seed),
            &denied_first
        )
        .unwrap_err(),
        Refusal::Access
    );
}

/// Configure separate reviewed promotions without weakening fetch denial.
fn with_promotion(action: &str) -> (Value, support::Catalog) {
    let (mut collection, mut catalog) = fixture();
    let mut decisions = support::value(&catalog, "decisions");
    decisions["entries"][0]["action"] = action.into();
    support::put(&mut catalog, "decisions", &decisions);
    let mut promotions = decisions;
    promotions["schema"] = "maestro-source-promotions/1".into();
    promotions["id"] = "promotions".into();
    promotions["entries"][0]["id"] = "promote-private".into();
    promotions["entries"][0]["action"] = "promote_knowledge".into();
    let reference = support::put(&mut catalog, "promotions", &promotions);
    let mut policy = support::value(&catalog, "policy");
    policy["sources"][0]["promotions"] = json!([reference]);
    support::put(&mut catalog, "policy", &policy);
    support::rebind(&mut collection, &mut catalog);
    (collection, catalog)
}

/// A promotion may change content disposition only after every denial succeeds.
#[test]
fn n07_promotions_are_separate_and_expiry_never_readmits() {
    for action in ["deny_fetch", "exclude_from_knowledge", "asset_only"] {
        let (mut collection, mut catalog) = with_promotion(action);
        let policy = checked(collection.clone(), &catalog).unwrap();
        let req = request("https://garden.example/docs/private", RequestKind::Resume);
        let result = admit(&policy, &req, &Controls::default());
        if action == "deny_fetch" {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().disposition(), Disposition::Knowledge);
        }
        if action != "deny_fetch" {
            for deny in ["caller", "network", "robots"] {
                let controls = Controls {
                    deny,
                    ..Controls::default()
                };
                assert!(admit(&policy, &req, &controls).is_err());
            }
        }
        let mut promotion = support::value(&catalog, "promotions");
        promotion["entries"][0]["expires_at"] = "2026-09-30T19:59:59Z".into();
        support::put(&mut catalog, "promotions", &promotion);
        support::rebind(&mut collection, &mut catalog);
        let policy = checked(collection, &catalog).unwrap();
        let result = admit(&policy, &req, &Controls::default());
        match action {
            "deny_fetch" => assert!(result.is_err()),
            "asset_only" => assert_eq!(result.unwrap().disposition(), Disposition::AssetOnly),
            _ => assert_eq!(result.unwrap().disposition(), Disposition::Excluded),
        }
    }
}

/// Full conjunctive selectors must not mistake absent attributes for a mismatch.
#[test]
fn n07_selection_is_conjunctive_and_unknown_denial_dimensions_hold() {
    let (mut collection, mut catalog) = fixture();
    let mut policy = support::value(&catalog, "policy");
    policy["sources"][0]["selectors"][0]["versions"] = json!(["v1", "v2"]);
    policy["sources"][0]["selectors"][0]["channels"] = json!(["stable"]);
    support::put(&mut catalog, "policy", &policy);
    support::rebind(&mut collection, &mut catalog);
    let policy = checked(collection.clone(), &catalog).unwrap();
    let mut req = request("https://garden.example/docs/start", RequestKind::Seed);
    assert!(admit(&policy, &req, &Controls::default()).is_err());
    req.attributes.version = Some("v2");
    req.attributes.channel = Some("stable");
    assert!(admit(&policy, &req, &Controls::default()).is_ok());
    req.attributes.channel = Some("beta");
    assert!(admit(&policy, &req, &Controls::default()).is_err());
    let mut decisions = support::value(&catalog, "decisions");
    decisions["entries"][0]["selector"]["path_prefix"] = "/docs".into();
    decisions["entries"][0]["selector"]["object_ids"] = json!(["secret"]);
    support::put(&mut catalog, "decisions", &decisions);
    support::rebind(&mut collection, &mut catalog);
    let policy = checked(collection, &catalog).unwrap();
    req.attributes.channel = Some("stable");
    assert!(admit(&policy, &req, &Controls::default()).is_err());
    req.attributes.object_id = Some("other");
    assert!(admit(&policy, &req, &Controls::default()).is_ok());
    req.attributes.object_id = Some("secret");
    assert!(admit(&policy, &req, &Controls::default()).is_err());
}

/// Migration bytes persist as immutable resources, never rewrite the old key.
pub(super) fn migration() -> Value {
    json!({
        "schema":"maestro-url-identity-migration/1", "id":"migration", "version":1,
        "collection_id":"garden", "visibility":"public",
        "scope_tags":["workspace/default/collection/garden"],
        "owner_ref":{
            "id":"owner",
            "digest":"0000000000000000000000000000000000000000000000000000000000000000"
        },
        "source_id":"notes", "old_version":1, "new_version":2,
        "entries":[{
            "old":"https://garden.example/docs/old",
            "new":"https://garden.example/docs/new"
        }]
    })
}
/// Bind a reviewed migration under the current source rule.
pub(super) fn with_migration(value: &mut Value) -> (Value, support::Catalog) {
    let (mut collection, mut catalog) = fixture();
    support::bind(value, &catalog);
    let reference = support::put(&mut catalog, "migration", value);
    let mut policy = support::value(&catalog, "policy");
    policy["sources"][0]["identity"]["version"] = 2.into();
    policy["sources"][0]["identity"]["migration"] = json!(reference);
    support::put(&mut catalog, "policy", &policy);
    support::rebind(&mut collection, &mut catalog);
    (collection, catalog)
}

/// Version/digest/source/scope/canonicality/chain guards reach the real resolver.
#[test]
fn n07_migrations_are_reviewed_versioned_and_never_silent() {
    let mut value = migration();
    let (collection, catalog) = with_migration(&mut value);
    let policy = checked(collection.clone(), &catalog).unwrap();
    let reference = catalog.0.get("migration").unwrap().reference.clone();
    assert_eq!(
        policy
            .identity_migrations()
            .get(&reference.id)
            .unwrap()
            .entries[0]
            .old,
        "https://garden.example/docs/old"
    );
    assert_eq!(
        admit(
            &policy,
            &request("https://garden.example/docs/old", RequestKind::Resume),
            &Controls::default()
        )
        .unwrap()
        .identity()
        .as_str(),
        "https://garden.example/docs/old"
    );
    let scratch = maestro_test_scratch::scratch_directory().unwrap();
    let store = Store::new(&scratch);
    assert_eq!(
        store
            .put(&catalog.0.get("migration").unwrap().bytes)
            .unwrap(),
        reference.digest
    );
    assert_eq!(
        store.get(&reference.digest).unwrap(),
        catalog.0.get("migration").unwrap().bytes
    );
    fs::remove_dir_all(scratch).unwrap();
    for (pointer, replacement) in [
        ("/schema", json!("maestro-url-identity-migration/2")),
        ("/id", json!("other")),
        ("/new_version", json!(3)),
        ("/old_version", json!(2)),
        ("/source_id", json!("other")),
        ("/scope_tags", json!(["workspace/secret"])),
        (
            "/entries/0/old",
            json!("https://user@garden.example/docs/old"),
        ),
        (
            "/entries/0/old",
            json!("https://garden.example/docs/old#part"),
        ),
        ("/entries/0/new", json!("https://other.example/docs/new")),
        (
            "/entries/0/new",
            json!("https://garden.example/docs-neighbour"),
        ),
        (
            "/entries/0/new",
            json!("https://garden.example/docs/private"),
        ),
        (
            "/entries/0/new",
            json!("https://garden.example/docs/new#part"),
        ),
        (
            "/entries/0/new",
            json!("https://garden.example/docs/new?unknown=1"),
        ),
    ] {
        let mut value = migration();
        *value.pointer_mut(pointer).unwrap() = replacement;
        let (collection, catalog) = with_migration(&mut value);
        assert!(
            checked(collection, &catalog).is_err(),
            "admitted mutation {pointer}"
        );
    }
    for entries in [
        json!([]),
        json!([
            {"old":"https://garden.example/docs/a","new":"https://garden.example/docs/b"},
            {"old":"https://garden.example/docs/a","new":"https://garden.example/docs/c"}
        ]),
        json!([
            {"old":"https://garden.example/docs/a","new":"https://garden.example/docs/b"},
            {"old":"https://garden.example/docs/b","new":"https://garden.example/docs/c"}
        ]),
        json!([{"old":"https://garden.example/docs/a","new":"https://garden.example/docs/a"}]),
    ] {
        let mut value = migration();
        value["entries"] = entries;
        let (collection, catalog) = with_migration(&mut value);
        assert!(checked(collection, &catalog).is_err());
    }
}
