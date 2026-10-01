//! Robots transport outcomes reach N10's actual fail-closed mapping.
use super::{n07_parse_url_identity_and_denial_precedence as n07, support};
use super::{n07_parse_url_identity_and_denial_precedence::Controls, n09_support::*};
use maestro_acquisition::Refusal;
use maestro_acquisition::{
    policy::{decision::admit, identity::FetchIdentity},
    transport::{
        http::Failure,
        robots::{DenyOverrides, RobotsBinding, RobotsCache},
        robots_store::RobotsStore,
        stream::Accounting,
    },
};
use serde_json::{Value, json};

#[test]
fn n09_robots_derived_admission_preserves_content_denial() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        assert!(
            admit(
                &policy,
                &fetch("https://garden.example/robots.txt").request,
                &controls
            )
            .is_err()
        );
        let wire = Wire::new(vec![response(
            200,
            "",
            b"User-agent: *\nDisallow: /docs/blocked\n",
        )]);
        let mut request = fetch("https://garden.example/robots.txt");
        request.robots = true;
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        let cache = http(&policy, &controls, &grants, &dns, &wire)
            .fetch_robots(&request, &mut accounting, 0)
            .await
            .unwrap();
        let allowed = FetchIdentity::parse(
            &policy.policy().sources[0],
            "https://garden.example/docs/start",
        )
        .unwrap();
        assert!(
            cache
                .check(
                    &allowed,
                    RobotsBinding::new(&policy, "notes").unwrap(),
                    0,
                    &DenyOverrides
                )
                .is_ok()
        );
        let denied = FetchIdentity::parse(
            &policy.policy().sources[0],
            "https://garden.example/docs/blocked",
        )
        .unwrap();
        assert!(
            cache
                .check(
                    &denied,
                    RobotsBinding::new(&policy, "notes").unwrap(),
                    0,
                    &DenyOverrides
                )
                .is_err()
        );
    });
}
#[test]
fn n09_robots_status_outcome_mapping_and_unread_404_410() {
    run(async {
        for (status, body, allowed) in [
            (404, vec![0; 512_001], true),
            (410, vec![0; 512_001], true),
            (500, vec![], false),
            (200, vec![0xff], false),
            (200, vec![b'x'; 512_001], false),
        ] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::new(vec![response(status, "", &body)]);
            let mut request = fetch("https://garden.example/robots.txt");
            request.robots = true;
            let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
            let cache = http(&policy, &controls, &grants, &dns, &wire)
                .fetch_robots(&request, &mut accounting, 0)
                .await
                .unwrap();
            let identity = FetchIdentity::parse(
                &policy.policy().sources[0],
                "https://garden.example/docs/start",
            )
            .unwrap();
            assert_eq!(
                cache
                    .check(
                        &identity,
                        RobotsBinding::new(&policy, "notes").unwrap(),
                        0,
                        &DenyOverrides
                    )
                    .is_ok(),
                allowed
            );
            if status == 404 || status == 410 {
                assert_eq!(accounting.wire_bytes(), 0);
            }
        }
    });
}
#[test]
fn n09_robots_redirect_scope_and_settings_refuse() {
    run(async {
        for (location, expected) in [
            ("/docs/start", Failure::Admission(Refusal::Access)),
            (
                "https://unknown.example/robots.txt",
                Failure::Admission(Refusal::Access),
            ),
        ] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::new(vec![response(
                302,
                &format!("Location: {location}\r\n"),
                b"",
            )]);
            let mut request = fetch("https://garden.example/robots.txt");
            request.robots = true;
            let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
            assert_eq!(
                http(&policy, &controls, &grants, &dns, &wire)
                    .fetch(&request, &mut accounting)
                    .await
                    .unwrap_err(),
                expected
            );
        }
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::default();
        let mut limits = policy.policy().sources[0].limits.clone();
        limits.redirects = 4.try_into().unwrap();
        let mut accounting = Accounting::new(limits);
        let mut request = fetch("https://garden.example/robots.txt");
        request.robots = true;
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&request, &mut accounting)
                .await
                .unwrap_err(),
            Failure::Configuration
        );
        assert_eq!(grants.calls.get(), 0);
    });
}
#[test]
fn n09_robots_store_bounds_and_digest_key() {
    let policies = [
        policy(),
        policy_with(|wire| wire["sources"][0]["robots"]["cache_ttl_ms"] = 2000.into()),
        policy_with(|wire| wire["sources"][0]["robots"]["cache_ttl_ms"] = 3000.into()),
    ];
    let identity = policies[0]
        .admit_robots("notes", "https://garden.example/robots.txt")
        .unwrap();
    let bindings = policies
        .each_ref()
        .map(|policy| RobotsBinding::new(policy, "notes").unwrap());
    let mut store = RobotsStore::new(2.try_into().unwrap());
    assert!(store.is_empty());
    for binding in bindings.iter().take(2) {
        store.insert(RobotsCache::response(&identity, *binding, 404, b"", 0));
    }
    assert_eq!(store.len(), 2);
    assert!(store.get(&identity, bindings[0]).is_some());
    assert!(store.get(&identity, bindings[1]).is_some());
    assert!(store.get(&identity, bindings[2]).is_none());
    store.insert(RobotsCache::response(&identity, bindings[1], 410, b"", 0));
    assert_eq!(store.len(), 2);
    assert!(store.get(&identity, bindings[0]).is_some());
    assert!(store.get(&identity, bindings[1]).is_some());
    store.insert(RobotsCache::response(&identity, bindings[2], 404, b"", 0));
    assert_eq!(store.len(), 2);
    assert!(store.get(&identity, bindings[0]).is_none());
    assert!(store.get(&identity, bindings[1]).is_some());
    let other = policies[0]
        .admit_robots("notes", "https://other.example/robots.txt")
        .unwrap();
    assert!(store.get(&other, bindings[1]).is_none());
}

#[test]
fn n09_robots_follows_five_freshly_admitted_redirects() {
    run(async {
        let policy = policy_with(|wire| {
            for n in 0..4 {
                let mut origin = wire["sources"][0]["origins"][0].clone();
                origin["id"] = format!("hop{n}").into();
                origin["host"] = format!("hop{n}.example").into();
                wire["sources"][0]["origins"]
                    .as_array_mut()
                    .unwrap()
                    .push(origin);
                let mut selector = wire["sources"][0]["selectors"][0].clone();
                selector["origin"] = format!("hop{n}").into();
                wire["sources"][0]["selectors"]
                    .as_array_mut()
                    .unwrap()
                    .push(selector);
            }
        });
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let mut responses = [
            "other.example",
            "hop0.example",
            "hop1.example",
            "hop2.example",
            "hop3.example",
        ]
        .iter()
        .map(|host| {
            response(
                302,
                &format!("Location: https://{host}/robots.txt\r\n"),
                b"",
            )
        })
        .collect::<Vec<_>>();
        responses.push(response(200, "", b"User-agent: *\nAllow: /\n"));
        let wire = Wire::new(responses);
        let mut request = fetch("https://garden.example/robots.txt");
        request.robots = true;
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        let result = http(&policy, &controls, &grants, &dns, &wire)
            .fetch(&request, &mut accounting)
            .await
            .unwrap();
        assert_eq!(result.status, 200);
        assert_eq!(grants.calls.get(), 6);
        assert_eq!(dns.calls.get(), 6);
    });
}

#[test]
fn n09_robots_read_budget_is_transport_failure() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", &vec![b'x'; 512_001])]);
        let mut request = fetch("https://garden.example/robots.txt");
        request.robots = true;
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&request, &mut accounting)
                .await
                .unwrap_err(),
            Failure::EncodedBytes
        );
        assert!(accounting.wire_bytes() <= 512_000);
    });
}

#[test]
fn n09_robots_transport_failure_maps_to_disallow_all() {
    run(async {
        for responses in [
            vec![],
            vec![response(302, "Location: /docs/start\r\n", b"")],
            vec![response(302, "Location: /robots.txt\r\n", b"")],
        ] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::new(responses);
            let mut request = fetch("https://garden.example/robots.txt");
            request.robots = true;
            let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
            let cache = http(&policy, &controls, &grants, &dns, &wire)
                .fetch_robots(&request, &mut accounting, 0)
                .await
                .unwrap();
            let identity = FetchIdentity::parse(
                &policy.policy().sources[0],
                "https://garden.example/docs/start",
            )
            .unwrap();
            assert!(
                cache
                    .check(
                        &identity,
                        RobotsBinding::new(&policy, "notes").unwrap(),
                        0,
                        &DenyOverrides
                    )
                    .is_err()
            );
        }
    });
}

#[test]
fn n09_robots_stricter_policy_redirect_ceiling_is_not_raised() {
    run(async {
        for source_bound in [false, true] {
            let policy = policy_with(|wire| tighten_redirects(wire, source_bound));
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::default();
            let mut request = fetch("https://garden.example/robots.txt");
            request.robots = true;
            let mut limits = policy.policy().sources[0].limits.clone();
            limits.redirects = 5.try_into().unwrap();
            let mut accounting = Accounting::new(limits);
            assert_eq!(
                http(&policy, &controls, &grants, &dns, &wire)
                    .fetch(&request, &mut accounting)
                    .await
                    .unwrap_err(),
                Failure::Configuration
            );
            assert_eq!(grants.calls.get(), 0);
            assert_eq!(dns.calls.get(), 0);
        }
    });
}

/// Tighten one immutable policy field without conflating settings validation.
#[expect(clippy::indexing_slicing, reason = "authored mutable fixture keys")]
fn tighten_redirects(wire: &mut Value, source_bound: bool) {
    let limits = if source_bound {
        &mut wire["sources"][0]["limits"]
    } else {
        &mut wire["aggregate_limits"]
    };
    limits["redirects"] = 4.into();
}

#[test]
fn n09_robots_agent_keys_coexist_with_one_policy_digest() {
    let policy = policy_with(|wire| {
        let mut source = wire["sources"][0].clone();
        source["id"] = "second".into();
        source["robots"]["agent"] = "Second".into();
        for selector in source["selectors"].as_array_mut().unwrap() {
            selector["source_id"] = "second".into();
        }
        wire["sources"].as_array_mut().unwrap().push(source);
    });
    let identity = policy
        .admit_robots("notes", "https://garden.example/robots.txt")
        .unwrap();
    let first = RobotsBinding::new(&policy, "notes").unwrap();
    let second = RobotsBinding::new(&policy, "second").unwrap();
    let mut store = RobotsStore::new(2.try_into().unwrap());
    store.insert(RobotsCache::response(&identity, first, 404, b"", 0));
    assert!(store.get(&identity, second).is_none());
    store.insert(RobotsCache::response(&identity, second, 410, b"", 0));
    assert_eq!(store.len(), 2);
    assert!(store.get(&identity, first).is_some());
    assert!(store.get(&identity, second).is_some());
}

#[test]
fn n09_robots_query_and_fragment_never_create_authority() {
    let policy =
        policy_with(|wire| wire["sources"][0]["identity"]["meaningful_queries"] = json!(["id"]));
    for suffix in ["?id=one", "#fragment"] {
        assert_eq!(
            policy
                .admit_robots(
                    "notes",
                    &format!("https://garden.example/robots.txt{suffix}")
                )
                .unwrap_err(),
            Refusal::Access
        );
    }
}

#[test]
fn n09_declared_but_unselected_origin_cannot_fetch_robots() {
    let policy = policy_with(|wire| {
        wire["sources"][0]["selectors"]
            .as_array_mut()
            .unwrap()
            .pop();
    });
    assert_eq!(
        policy
            .admit_robots("notes", "https://other.example/robots.txt")
            .unwrap_err(),
        Refusal::Access
    );
}

#[test]
fn n09_origin_fetch_denial_also_denies_derived_robots() {
    let (mut collection, mut catalog) = n07::fixture();
    let mut decisions = support::value(&catalog, "decisions");
    decisions["entries"][0]["selector"]["path_prefix"] = Value::Null;
    support::put(&mut catalog, "decisions", &decisions);
    support::rebind(&mut collection, &mut catalog);
    let checked = n07::checked(collection, &catalog).unwrap();
    assert_eq!(
        checked
            .admit_robots("notes", "https://garden.example/robots.txt")
            .unwrap_err(),
        Refusal::Access
    );
}

#[test]
fn n09_robots_origin_keys_coexist_with_same_agent_and_digest() {
    let policy = policy();
    let binding = RobotsBinding::new(&policy, "notes").unwrap();
    let garden = policy
        .admit_robots("notes", "https://garden.example/robots.txt")
        .unwrap();
    let other = policy
        .admit_robots("notes", "https://other.example/robots.txt")
        .unwrap();
    let mut store = RobotsStore::new(2.try_into().unwrap());
    store.insert(RobotsCache::response(&garden, binding, 404, b"", 0));
    store.insert(RobotsCache::response(&other, binding, 410, b"", 0));
    assert_eq!(store.len(), 2);
    assert!(store.get(&garden, binding).is_some());
    assert!(store.get(&other, binding).is_some());
}
