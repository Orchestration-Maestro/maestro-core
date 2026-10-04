//! RFC 9309 regression assertions from the independent N10 review probes.
use super::{
    n07_parse_url_identity_and_denial_precedence as n07,
    n10_conform_robots_and_aggregate_origin_pacing::{binding, checked_robots, identity, source},
    support,
};
use maestro_acquisition::{
    Refusal,
    policy::identity::FetchIdentity,
    transport::robots::{DenyOverrides, Rfc9309, RobotsCache, RobotsRules},
};

#[test]
fn n10_encoded_star_is_literal() {
    let source = source();
    let rules = Rfc9309::parse(
        b"User-agent: *\nDisallow: /docs/file-with-a-%2A.html\n",
        512_000,
    )
    .unwrap();
    for (path, allowed) in [
        ("/docs/file-with-a-*.html", false),
        ("/docs/file-with-a-%2A.html", false),
        ("/docs/file-with-a-x.html", true),
    ] {
        assert_eq!(
            rules.allowed("Maestro", &identity(&source, path)),
            allowed,
            "{path}"
        );
    }
}

#[test]
fn n10_encoded_dollar_is_literal() {
    let source = source();
    let rules = Rfc9309::parse(b"User-agent: *\nDisallow: /docs/foo-%24\n", 512_000).unwrap();
    for (path, allowed) in [
        ("/docs/foo-$", false),
        ("/docs/foo-%24", false),
        ("/docs/foo-$/more", false),
        ("/docs/foo-x", true),
    ] {
        assert_eq!(
            rules.allowed("Maestro", &identity(&source, path)),
            allowed,
            "{path}"
        );
    }
}

#[test]
fn n10_unicode_whitespace_is_path_data() {
    let source = source();
    for (whitespace, encoded) in [
        ('\u{00a0}', "%C2%A0"),
        ('\u{2003}', "%E2%80%83"),
        ('\u{202f}', "%E2%80%AF"),
    ] {
        let rules = Rfc9309::parse(
            format!("\tUser-agent: * \t\nDisallow: /docs/a\nAllow: /docs/a{whitespace}\n")
                .as_bytes(),
            512_000,
        )
        .unwrap();
        assert!(!rules.allowed("Maestro", &identity(&source, "/docs/a")));
        assert!(rules.allowed("Maestro", &identity(&source, &format!("/docs/a{encoded}"))));
    }
}

#[test]
fn n10_dollar_before_nbsp_is_not_an_anchor() {
    let source = source();
    let rules = Rfc9309::parse(
        "User-agent: *\nDisallow: /docs/a$\u{00a0}\n".as_bytes(),
        512_000,
    )
    .unwrap();
    assert!(!rules.allowed("Maestro", &identity(&source, "/docs/a$%C2%A0")));
    assert!(rules.allowed("Maestro", &identity(&source, "/docs/a$")));
}

#[test]
fn n10_robots_uri_is_implicitly_allowed_by_matcher() {
    let source = source();
    let rules = Rfc9309::parse(b"User-agent: *\nDisallow: /\n", 512_000).unwrap();
    assert!(rules.allowed("Maestro", &identity(&source, "/robots.txt")));
    assert!(!rules.allowed("Maestro", &identity(&source, "/docs/a")));
    assert!(!rules.allowed("Maestro", &identity(&source, "/robots.txt/child")));
    assert!(!rules.allowed("Bad/Token", &identity(&source, "/robots.txt")));
}

#[test]
fn n10_robots_uri_can_refresh_denied_or_missing_rules() {
    let source = source();
    let checked = checked_robots(&source.robots);
    let bound = binding(&checked);
    let target = identity(&source, "/robots.txt");
    for cache in [
        RobotsCache::response(&target, bound, 503, b"", 0),
        RobotsCache::response(&target, bound, 200, b"User-agent: *\nDisallow: /\n", 0),
        RobotsCache::unreadable(&target, bound, 0),
    ] {
        assert_eq!(cache.check(&target, bound, 0, &DenyOverrides), Ok(()));
        assert_eq!(cache.check(&target, bound, 1000, &DenyOverrides), Ok(()));
        assert_eq!(
            cache.check(&identity(&source, "/docs/a"), bound, 0, &DenyOverrides),
            Err(Refusal::Access)
        );
    }
}

#[test]
fn n10_cr_and_mixed_line_endings_are_records() {
    let source = source();
    for body in [
        "User-agent: *\nDisallow: /docs/secret\rAllow: /docs/public\r",
        "User-agent: *\rDisallow: /docs/secret\rAllow: /docs/public\r",
        "User-agent: *\r\nDisallow: /docs/secret\rAllow: /docs/public\n",
    ] {
        let rules = Rfc9309::parse(body.as_bytes(), 512_000).unwrap();
        assert!(!rules.allowed("Maestro", &identity(&source, "/docs/secret")));
        assert!(rules.allowed("Maestro", &identity(&source, "/docs/public")));
    }
}

#[test]
fn n10_status_mapping_never_bypasses_body_cap() {
    let mut source = source();
    source.robots.rules_max_bytes = u64::MAX.try_into().unwrap();
    let checked = checked_robots(&source.robots);
    let bound = binding(&checked);
    let target = identity(&source, "/docs/a");
    let body = vec![b' '; 512_001];
    for status in [404, 410, 200] {
        let cache = RobotsCache::response(&target, bound, status, &body, 0);
        assert_eq!(
            cache.check(&target, bound, 0, &DenyOverrides),
            Err(Refusal::Access),
            "{status}"
        );
    }
    let cache = RobotsCache::response(&target, bound, 404, b"not rules", 0);
    assert_eq!(cache.check(&target, bound, 0, &DenyOverrides), Ok(()));
    source.robots.rules_max_bytes = 10.try_into().unwrap();
    let tight = checked_robots(&source.robots);
    let bound = binding(&tight);
    let cache = RobotsCache::response(&target, bound, 410, b"elevenbytes", 0);
    assert_eq!(
        cache.check(&target, bound, 0, &DenyOverrides),
        Err(Refusal::Access)
    );
}

#[test]
fn n10_policy_digest_changes_miss_and_entries_coexist() {
    let (mut collection, mut catalog) = n07::fixture();
    let old = n07::checked(collection.clone(), &catalog).unwrap();
    let mut policy = support::value(&catalog, "policy");
    let requests = policy["aggregate_limits"]["requests"].as_u64().unwrap();
    policy["aggregate_limits"]["requests"] = (requests + 1).into();
    support::put(&mut catalog, "policy", &policy);
    support::rebind(&mut collection, &mut catalog);
    let new = n07::checked(collection, &catalog).unwrap();
    assert_ne!(old.reference().digest, new.reference().digest);
    let old_source = old.policy().sources.first().unwrap();
    let target = FetchIdentity::parse(old_source, "https://garden.example/docs/a").unwrap();
    let old_cache = RobotsCache::response(&target, binding(&old), 200, b"", 0);
    let new_cache = RobotsCache::response(&target, binding(&new), 200, b"", 0);
    assert_eq!(
        old_cache.check(&target, binding(&new), 0, &DenyOverrides),
        Err(Refusal::Access)
    );
    assert_eq!(
        new_cache.check(&target, binding(&old), 0, &DenyOverrides),
        Err(Refusal::Access)
    );
    assert_eq!(
        old_cache.check(&target, binding(&old), 0, &DenyOverrides),
        Ok(())
    );
    assert_eq!(
        new_cache.check(&target, binding(&new), 0, &DenyOverrides),
        Ok(())
    );
}
