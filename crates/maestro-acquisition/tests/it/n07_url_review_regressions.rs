//! N07 review regressions share the checked synthetic policy closure.
use super::{
    n07_parse_url_identity_and_denial_precedence::{self as n07, Controls},
    support,
};
use maestro_acquisition::policy::{
    decision::{RequestKind, admit},
    identity::{DisplayLink, FetchIdentity, SignedTransferUrl},
};
use maestro_acquisition::{Refusal, parse_policy};
use serde_json::json;

/// Each protected-reference constructor must enforce the shared unsafe-URL boundary.
#[test]
fn n07_protected_references_reject_unsafe_urls() {
    for url in [
        "http://garden.example/docs",
        "https://user@garden.example/docs",
        "https://garden.example/docs\n/start",
        "https://garden.example/docs/%01",
        "https://garden.example/docs/%zz",
        "https://garden.example/docs/%",
    ] {
        assert_eq!(
            DisplayLink::parse(url).unwrap_err(),
            Refusal::Invalid,
            "display admitted {url:?}"
        );
        assert_eq!(
            SignedTransferUrl::parse(url).unwrap_err(),
            Refusal::Invalid,
            "transfer admitted {url:?}"
        );
    }
}

/// Review probe 1: percent-encoded unreserved characters must not dodge a denial.
#[test]
fn n07_review_encoded_unreserved_path_cannot_bypass_denial() {
    let (collection, catalog) = n07::fixture();
    let policy = n07::checked(collection, &catalog).unwrap();
    assert_eq!(
        admit(
            &policy,
            &n07::request("https://garden.example/docs/private", RequestKind::Seed),
            &Controls::default()
        )
        .unwrap_err(),
        Refusal::Access
    );
    let mut admitted = Vec::new();
    for kind in [
        RequestKind::Seed,
        RequestKind::Redirect,
        RequestKind::Subresource,
        RequestKind::Resume,
        RequestKind::Retry,
    ] {
        for url in [
            "https://garden.example/docs/%70rivate",
            "https://garden.example/docs/pri%76ate",
        ] {
            let result = admit(&policy, &n07::request(url, kind), &Controls::default());
            eprintln!("N07_REVIEW probe1 {kind:?} {url}: {result:?}");
            if !matches!(result, Err(Refusal::Invalid)) {
                admitted.push((kind, url));
            }
        }
    }
    assert!(
        admitted.is_empty(),
        "admitted encoded spellings of a denied path: {admitted:?}"
    );
}

/// Review probe 2: duplicate, chained and self-cycling keys after `:443` normalization.
#[test]
fn n07_review_migration_keys_compare_canonical_identities() {
    let cases = [
        (
            "a-duplicate-old",
            json!([
                {"old": "https://garden.example/docs/a", "new": "https://garden.example/docs/b"},
                {"old": "https://garden.example:443/docs/a", "new": "https://garden.example/docs/c"}
            ]),
        ),
        (
            "b-chain",
            json!([
                {"old": "https://garden.example/docs/a", "new": "https://garden.example/docs/b"},
                {"old": "https://garden.example:443/docs/b", "new": "https://garden.example/docs/c"}
            ]),
        ),
        (
            "c-self-cycle",
            json!([
                {"old": "https://garden.example:443/docs/a", "new": "https://garden.example/docs/a"}
            ]),
        ),
        (
            "d-noncanonical-old",
            json!([
                {"old": "https://garden.example:443/docs/a", "new": "https://garden.example/docs/b"}
            ]),
        ),
    ];
    for (name, entries) in cases {
        let mut value = n07::migration();
        value["entries"] = entries;
        let (collection, catalog) = n07::with_migration(&mut value);
        assert_eq!(
            n07::checked(collection, &catalog).unwrap_err(),
            Refusal::Invalid,
            "accepted non-canonical migration case {name}"
        );
    }
}

/// Unreserved bytes have one raw spelling; policy prefixes share the URL guard.
#[test]
fn n07_encoded_unreserved_paths_and_policy_prefixes_refuse() {
    let (collection, catalog) = n07::fixture();
    let policy = n07::checked(collection, &catalog).unwrap();
    for byte in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~" {
        let path = format!("/docs/%{byte:02X}private");
        assert_eq!(
            FetchIdentity::parse(
                &policy.policy().sources[0],
                &format!("https://garden.example{path}")
            )
            .unwrap_err(),
            Refusal::Invalid,
            "encoded unreserved byte {byte}"
        );
        for pointer in [
            "/sources/0/origins/0/path_prefixes/0",
            "/sources/0/selectors/0/path_prefix",
        ] {
            let mut value = support::value(&catalog, "policy");
            *value.pointer_mut(pointer).unwrap() = path.clone().into();
            assert!(
                parse_policy(&value.to_string()).is_err(),
                "admitted {pointer}: {path}"
            );
        }
    }
    for path in ["/docs/%2Fprivate", "/docs/%2fprivate"] {
        assert!(
            FetchIdentity::parse(
                &policy.policy().sources[0],
                &format!("https://garden.example{path}")
            )
            .is_err()
        );
    }
}

/// Reserved/UTF-8 escape hex case is one path identity, never a denial bypass.
#[test]
fn n07_path_escape_case_is_canonical_and_denial_first() {
    let (collection, catalog) = n07::fixture();
    let checked = n07::checked(collection, &catalog).unwrap();
    let mut source = checked.policy().sources[0].clone();
    source.identity.meaningful_queries = vec!["v".into()];
    for (upper, lower) in [("%3A", "%3a"), ("%E2%80%99", "%e2%80%99")] {
        let url = format!("https://garden.example/docs/{upper}");
        let lowercase = format!("https://garden.example/docs/{lower}");
        let identity = FetchIdentity::parse(&source, &url).unwrap();
        assert_eq!(identity, FetchIdentity::parse(&source, &lowercase).unwrap());
        assert_eq!(identity.as_str(), url);
        assert_eq!(
            FetchIdentity::parse(&source, &format!("{lowercase}?v=%3a%70"))
                .unwrap()
                .as_str(),
            format!("{url}?v=%3a%70")
        );
        assert!(
            admit(
                &checked,
                &n07::request(&lowercase, RequestKind::Seed),
                &Controls::default()
            )
            .is_ok()
        );
    }
    for (prefix, suffix) in [("%3A", "%3a"), ("%3a", "%3A")] {
        let (mut collection, mut catalog) = n07::fixture();
        let mut value = support::value(&catalog, "policy");
        let path = format!("/docs/{prefix}");
        value["sources"][0]["origins"][0]["path_prefixes"] = json!([path]);
        value["sources"][0]["selectors"][0]["path_prefix"] = path.clone().into();
        value["sources"][0]["seeds"] = json!([format!("https://garden.example{path}/start")]);
        support::put(&mut catalog, "policy", &value);
        let mut decisions = support::value(&catalog, "decisions");
        decisions["entries"][0]["selector"]["path_prefix"] = path.into();
        support::put(&mut catalog, "decisions", &decisions);
        support::rebind(&mut collection, &mut catalog);
        let policy = n07::checked(collection, &catalog).unwrap();
        let source = &policy.policy().sources[0];
        assert_eq!(source.origins[0].path_prefixes, ["/docs/%3A"]);
        assert_eq!(
            source.selectors[0].path_prefix.as_deref(),
            Some("/docs/%3A")
        );
        for kind in [
            RequestKind::Seed,
            RequestKind::Redirect,
            RequestKind::Subresource,
            RequestKind::Resume,
            RequestKind::Retry,
        ] {
            let url = format!("https://garden.example/docs/{suffix}/private");
            assert_eq!(
                admit(&policy, &n07::request(&url, kind), &Controls::default()).unwrap_err(),
                Refusal::Access
            );
        }
    }
}

/// Historical query semantics are not reinterpreted using the replacement rule.
#[test]
fn n07_migration_preserves_historical_queries() {
    let mut value = n07::migration();
    value["entries"][0]["old"] = "https://garden.example/docs/old?unknown=%70%3a".into();
    let (collection, catalog) = n07::with_migration(&mut value);
    let policy = n07::checked(collection, &catalog).unwrap();
    assert_eq!(
        policy.identity_migrations()["migration"].entries[0].old,
        "https://garden.example/docs/old?unknown=%70%3a"
    );
}
