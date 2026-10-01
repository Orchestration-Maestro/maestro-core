//! N07 edge guards share the primary task's synthetic fixture builders.
use super::{
    n07_parse_url_identity_and_denial_precedence::{self as n07, Controls},
    support,
};
use maestro_acquisition::policy::{
    decision::{Disposition, ItemAttributes, RequestKind, admit},
    identity::{FetchIdentity, IdentityMigration},
    resolve::parse_resource,
};
use maestro_acquisition::{
    AdmissionStatus, DirectFiles, LocalResource, PolicySource, Refusal, parse_policy,
};
use maestro_kernel::artifact::Digest;
use maestro_knowledge::collection::Declaration;
use maestro_test_scratch::scratch_directory;
use serde_json::json;
use std::{collections::BTreeMap, fs};

/// Current time and every selector dimension are checked; expiry holds denials.
#[test]
fn n07_current_time_selection_and_cache_false() {
    let (mut collection, mut catalog) = n07::fixture();
    let mut decisions = support::value(&catalog, "decisions");
    decisions["entries"][0]["expires_at"] = "2026-09-29T00:00:00Z".into();
    support::put(&mut catalog, "decisions", &decisions);
    support::rebind(&mut collection, &mut catalog);
    let policy = n07::checked(collection.clone(), &catalog).unwrap();
    let mut req = n07::request("https://garden.example/docs/private", RequestKind::Seed);
    assert!(admit(&policy, &req, &Controls::default()).is_err());
    req.url = "https://garden.example/docs/start";
    req.cache_bypass = false;
    assert!(
        !admit(&policy, &req, &Controls::default())
            .unwrap()
            .cache_bypass()
    );
    req.now = "invalid";
    assert_eq!(
        admit(&policy, &req, &Controls::default()).unwrap_err(),
        Refusal::Invalid
    );
    req.now = "2026-09-30T20:00:00Z";
    req.source_id = "unknown";
    assert!(admit(&policy, &req, &Controls::default()).is_err());
    req.source_id = "notes";
    for dimension in ["object_ids", "media_types"] {
        req.attributes = ItemAttributes::default();
        let mut value = support::value(&catalog, "policy");
        value["sources"][0]["selectors"][0][dimension] = json!(["known"]);
        support::put(&mut catalog, "policy", &value);
        support::rebind(&mut collection, &mut catalog);
        let policy = n07::checked(collection.clone(), &catalog).unwrap();
        assert!(admit(&policy, &req, &Controls::default()).is_err());
        req.attributes.object_id = Some("known");
        req.attributes.media_type = Some("known");
        assert!(admit(&policy, &req, &Controls::default()).is_ok());
    }
}

/// Explicit sorting/tracking changes require versioned identities, not aliases.
#[test]
fn n07_identity_namespace_tracking_and_duplicate_rule_names() {
    let (_, catalog) = n07::fixture();
    let mut source = parse_policy(&support::value(&catalog, "policy").to_string())
        .unwrap()
        .sources
        .remove(0);
    source.identity.ignored_tracking_queries = vec!["track".into()];
    let plain = FetchIdentity::parse(&source, "https://garden.example/docs").unwrap();
    assert_eq!(
        plain,
        FetchIdentity::parse(&source, "https://garden.example/docs?track=secret").unwrap()
    );
    source.id = "other".into();
    assert_ne!(
        plain,
        FetchIdentity::parse(&source, "https://garden.example/docs").unwrap()
    );
    source.identity.version = 2.try_into().unwrap();
    assert_eq!(
        FetchIdentity::parse(&source, "https://garden.example/docs")
            .unwrap()
            .version()
            .get(),
        2
    );
    source.identity.meaningful_queries = vec!["v".into(), "v".into()];
    assert!(FetchIdentity::parse(&source, "https://garden.example/docs").is_err());
    source.identity.meaningful_queries.clear();
    source
        .identity
        .ignored_tracking_queries
        .push("track".into());
    assert!(FetchIdentity::parse(&source, "https://garden.example/docs").is_err());
}

/// Reviewed migration survives local file reload; trust and strict counts refuse.
#[test]
fn n07_migration_persistence_and_resource_trust() {
    let (collection, mut catalog) = n07::with_migration(&mut n07::migration());
    let scratch = scratch_directory().unwrap();
    let mut bindings = BTreeMap::new();
    for (id, resource) in &catalog.0 {
        let path = scratch.join(format!("{id}.json"));
        fs::write(&path, &resource.bytes).unwrap();
        bindings.insert(
            id.clone(),
            LocalResource {
                path,
                admission: resource.admission.clone(),
            },
        );
    }
    let declaration: Declaration = serde_json::from_value(collection.clone()).unwrap();
    let files = DirectFiles::new(bindings);
    let grants = support::scopes();
    let principal = support::principal(&grants);
    let first = files.resolve(&declaration, &principal).unwrap();
    let replay = files.resolve(&declaration, &principal).unwrap();
    assert_eq!(first.identity_migrations(), replay.identity_migrations());
    assert_eq!(first.identity_migrations().len(), 1);
    fs::remove_dir_all(scratch).unwrap();
    for status in [
        AdmissionStatus::Proposed,
        AdmissionStatus::Held,
        AdmissionStatus::Revoked,
    ] {
        catalog.0.get_mut("migration").unwrap().admission.status = status;
        assert_eq!(
            n07::checked(collection.clone(), &catalog).unwrap_err(),
            Refusal::Unqualified
        );
    }
    catalog.0.get_mut("migration").unwrap().admission.status = AdmissionStatus::Reviewed;
    catalog.0.get_mut("migration").unwrap().admission.digest = Digest::of(b"wrong");
    assert_eq!(
        n07::checked(collection, &catalog).unwrap_err(),
        Refusal::Digest
    );
    let mut too_many = n07::migration();
    too_many["entries"] = json!(vec![
        json!({"old":"https://a.example/a", "new":"https://a.example/b"});
        10_001
    ]);
    assert!(parse_resource::<IdentityMigration>(&serde_json::to_vec(&too_many).unwrap()).is_err());
}

/// An active promotion cannot defeat robots; expired/future promotion stays held.
#[test]
fn n07_promotion_time_boundaries_preserve_disposition() {
    let (mut collection, mut catalog) = n07::fixture();
    let mut decisions = support::value(&catalog, "decisions");
    decisions["entries"][0]["action"] = "asset_only".into();
    support::put(&mut catalog, "decisions", &decisions);
    let mut promotion = decisions;
    promotion["schema"] = "maestro-source-promotions/1".into();
    promotion["id"] = "promotion".into();
    promotion["entries"][0]["action"] = "promote_knowledge".into();
    for (effective, expires, expected) in [
        (
            "2026-09-30T20:00:00.000Z",
            "2026-09-30T20:00:01Z",
            Disposition::Knowledge,
        ),
        (
            "2026-09-30T20:00:00.1Z",
            "2026-09-30T20:00:01Z",
            Disposition::AssetOnly,
        ),
        (
            "2026-09-30T19:00:00Z",
            "2026-09-30T20:00:00.000Z",
            Disposition::AssetOnly,
        ),
    ] {
        promotion["entries"][0]["effective_at"] = effective.into();
        promotion["entries"][0]["expires_at"] = expires.into();
        let reference = support::put(&mut catalog, "promotion", &promotion);
        let mut policy = support::value(&catalog, "policy");
        policy["sources"][0]["promotions"] = json!([reference]);
        support::put(&mut catalog, "policy", &policy);
        support::rebind(&mut collection, &mut catalog);
        let policy = n07::checked(collection.clone(), &catalog).unwrap();
        assert_eq!(
            admit(
                &policy,
                &n07::request("https://garden.example/docs/private", RequestKind::Resume),
                &Controls::default()
            )
            .unwrap()
            .disposition(),
            expected
        );
    }
}

/// Selector source and origin names do not leak denial to an adjacent namespace.
#[test]
fn n07_selector_source_origin_and_path_boundaries() {
    let (mut collection, mut catalog) = n07::fixture();
    let mut policy = support::value(&catalog, "policy");
    let mut origin = policy["sources"][0]["origins"][0].clone();
    origin["id"] = "assets".into();
    origin["host"] = "assets.example".into();
    policy["sources"][0]["origins"]
        .as_array_mut()
        .unwrap()
        .push(origin);
    support::put(&mut catalog, "policy", &policy);
    support::rebind(&mut collection, &mut catalog);
    let checked = n07::checked(collection.clone(), &catalog).unwrap();
    let mut req = n07::request("https://assets.example/docs/private", RequestKind::Seed);
    assert!(admit(&checked, &req, &Controls::default()).is_err());
    let mut policy = support::value(&catalog, "policy");
    let mut selection = policy["sources"][0]["selectors"][0].clone();
    selection["origin"] = "assets".into();
    policy["sources"][0]["selectors"]
        .as_array_mut()
        .unwrap()
        .push(selection);
    let mut other = policy["sources"][0].clone();
    other["id"] = "other".into();
    for selector in other["selectors"].as_array_mut().unwrap() {
        selector["source_id"] = "other".into();
    }
    policy["sources"].as_array_mut().unwrap().push(other);
    support::put(&mut catalog, "policy", &policy);
    support::rebind(&mut collection, &mut catalog);
    let checked = n07::checked(collection, &catalog).unwrap();
    assert!(admit(&checked, &req, &Controls::default()).is_ok());
    req.url = "https://garden.example/docs/private";
    req.source_id = "other";
    assert!(admit(&checked, &req, &Controls::default()).is_ok());
}

/// Raw supplied query/path spellings cannot be silently parser-normalized.
#[test]
fn n07_parser_normalization_and_selector_encoding() {
    let (_, catalog) = n07::fixture();
    for suffix in ["/docs?x='", "/docs/é", "/docs?x=é"] {
        let mut policy = support::value(&catalog, "policy");
        policy["sources"][0]["identity"]["meaningful_queries"] = json!(["x"]);
        policy["sources"][0]["seeds"][0] = format!("https://garden.example{suffix}").into();
        assert!(parse_policy(&policy.to_string()).is_err());
    }
    for prefix in ["/docs/%", "/docs/%zz", "/docs/%01", "/docs//start"] {
        let mut policy = support::value(&catalog, "policy");
        policy["sources"][0]["selectors"][0]["path_prefix"] = prefix.into();
        assert!(parse_policy(&policy.to_string()).is_err());
    }
}

/// Future exclusion becomes effective at its exact timestamp, not first parse.
#[test]
fn n07_decision_effective_time_is_rechecked() {
    let (mut collection, mut catalog) = n07::fixture();
    let mut decisions = support::value(&catalog, "decisions");
    decisions["entries"][0]["effective_at"] = "2026-09-30T20:01:00Z".into();
    support::put(&mut catalog, "decisions", &decisions);
    support::rebind(&mut collection, &mut catalog);
    let policy = n07::checked(collection, &catalog).unwrap();
    let mut req = n07::request("https://garden.example/docs/private", RequestKind::Seed);
    assert!(admit(&policy, &req, &Controls::default()).is_ok());
    req.now = "2026-09-30T20:01:00Z";
    assert!(admit(&policy, &req, &Controls::default()).is_err());
}

/// Unknown promotion metadata never changes eligibility; missing metadata holds.
#[test]
fn n07_unknown_promotion_and_empty_query_names_hold() {
    let (mut collection, mut catalog) = n07::fixture();
    let mut decisions = support::value(&catalog, "decisions");
    decisions["entries"][0]["action"] = "asset_only".into();
    support::put(&mut catalog, "decisions", &decisions);
    let mut promotion = decisions;
    promotion["schema"] = "maestro-source-promotions/1".into();
    promotion["id"] = "promotion".into();
    promotion["entries"][0]["action"] = "promote_knowledge".into();
    promotion["entries"][0]["selector"]["object_ids"] = json!(["known"]);
    let reference = support::put(&mut catalog, "promotion", &promotion);
    let mut value = support::value(&catalog, "policy");
    value["sources"][0]["promotions"] = json!([reference]);
    support::put(&mut catalog, "policy", &value);
    support::rebind(&mut collection, &mut catalog);
    let policy = n07::checked(collection, &catalog).unwrap();
    let mut req = n07::request("https://garden.example/docs/private", RequestKind::Seed);
    assert_eq!(
        admit(&policy, &req, &Controls::default())
            .unwrap()
            .disposition(),
        Disposition::AssetOnly
    );
    req.attributes.object_id = Some("known");
    assert_eq!(
        admit(&policy, &req, &Controls::default())
            .unwrap()
            .disposition(),
        Disposition::Knowledge
    );
    let mut source = policy.policy().sources[0].clone();
    source.identity.meaningful_queries = vec![String::new()];
    assert!(FetchIdentity::parse(&source, "https://garden.example/docs?=1").is_err());
}

/// A replacement URL must pass source selection, not just the origin envelope.
#[test]
fn n07_migration_rechecks_source_selection() {
    let (mut collection, mut catalog) = n07::with_migration(&mut n07::migration());
    let mut value = support::value(&catalog, "policy");
    value["sources"][0]["selectors"][0]["path_prefix"] = "/docs/old".into();
    support::put(&mut catalog, "policy", &value);
    support::rebind(&mut collection, &mut catalog);
    assert!(n07::checked(collection, &catalog).is_err());
}

/// Ambiguous origin labels cannot select whichever declaration hides a denial.
#[test]
fn n07_overlapping_origin_labels_refuse_instead_of_first_match() {
    let (_, catalog) = n07::fixture();
    let mut policy = support::value(&catalog, "policy");
    let mut origin = policy["sources"][0]["origins"][0].clone();
    origin["id"] = "shadow".into();
    policy["sources"][0]["origins"]
        .as_array_mut()
        .unwrap()
        .push(origin);
    assert!(parse_policy(&policy.to_string()).is_err());
}
