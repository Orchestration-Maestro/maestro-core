//! Independently authored RFC 9309 and shared-origin contracts.
use super::support;
use maestro_acquisition::policy::{identity::FetchIdentity, source::Source};
use maestro_acquisition::transport::{
    pacing::{Demand, OriginLedger, OriginPermit, PacingLimits, Pending},
    robots::{
        DenyOverrides, OverrideDecision, OverrideRequest, Rfc9309, RobotsCache, RobotsOverride,
        RobotsRules,
    },
};
use maestro_acquisition::{Ref, Refusal, parse_policy};
use maestro_kernel::artifact::Digest;
use std::{
    sync::{Arc, Barrier},
    thread,
};

/// Synthetic source, with the same URL parser production uses.
pub(super) fn source() -> Source {
    let (_, catalog) = support::fixture();
    let mut source = parse_policy(&support::value(&catalog, "policy").to_string())
        .unwrap()
        .sources
        .remove(0);
    source
        .origins
        .iter_mut()
        .for_each(|origin| origin.host = "docs.example.test".into());
    source.robots.agent = "Maestro".into();
    source.robots.cache_ttl_ms = 1000.try_into().unwrap();
    source
        .origins
        .iter_mut()
        .for_each(|origin| origin.path_prefixes = vec!["/".into()]);
    source
}
/// Parse a destination under explicit policy, not arbitrary URL text.
pub(super) fn identity(source: &Source, path: &str) -> FetchIdentity {
    FetchIdentity::parse(source, &format!("https://docs.example.test{path}")).unwrap()
}
/// Two bounds deliberately conflict: intervals are floors, concurrency ceilings.
pub(super) fn limits(interval: u64, concurrency: u64) -> PacingLimits {
    let mut limits = source().limits;
    limits.origin_interval_ms = interval.try_into().unwrap();
    limits.origin_concurrency = concurrency.try_into().unwrap();
    limits.requests = 20.try_into().unwrap();
    limits.elapsed_ms = 100_000.try_into().unwrap();
    limits.max_backoff_ms = 60_000;
    limits.retries = 2;
    PacingLimits::compose([&limits]).unwrap()
}
/// The five real policy levels must compose without precedence shortcuts.
#[test]
fn n10_floor_max_ceiling_min_every_order() {
    let mut levels = vec![source().limits; 5];
    for (index, level) in levels.iter_mut().enumerate() {
        level.origin_interval_ms = if index == 2 { 1000 } else { 100 }.try_into().unwrap();
        level.origin_concurrency = if index == 3 { 1 } else { 4 }.try_into().unwrap();
    }
    for _ in 0..5 {
        let effective = PacingLimits::compose(levels.iter()).unwrap();
        assert_eq!(effective.interval_ms(), 1000);
        assert_eq!(effective.concurrency(), 1);
        levels.rotate_left(1);
    }
    assert_eq!(PacingLimits::compose([]), Err(Refusal::Missing));
}

#[test]
fn n10_rfc_groups_merge_and_allow_ties() {
    let rules = Rfc9309::parse(
        b"User-agent: *\n\
        Disallow: /\n\
        User-agent: maestro\n\
        Disallow: /docs\n\
        Allow: /docs/public\n\
        User-agent: MAESTRO\n\
        Allow: /docs/tie\n\
        Disallow: /docs/tie\n\
        ",
        512_000,
    )
    .unwrap();
    let source = source();
    for (path, allowed) in [
        ("/docs", false),
        ("/docs/private", false),
        ("/docs/public", true),
        ("/docs/tie", true),
        ("/other", true),
    ] {
        assert_eq!(
            rules.allowed("Maestro", &identity(&source, path)),
            allowed,
            "{path}"
        );
    }
    assert!(!rules.allowed("OtherBot", &identity(&source, "/other")));
    // Product tokens match whole tokens, not arbitrary substrings.
    assert!(!rules.allowed("SuperMaestro", &identity(&source, "/other")));
}

#[test]
fn n10_rfc_wildcards_octets_and_comments() {
    let rules = Rfc9309::parse(
        "User-agent: Maestro\n\
        Disallow: /docs/*.pdf$ # files only\n\
        Disallow: /%C3%A9\n\
        Disallow: /%62locked\n\
        Disallow: /docs/%3asecret\n\
        Allow: /docs/*.pdf/public\n\
        Disallow:\n\
        Sitemap: https://example.test/map\n\
        "
        .as_bytes(),
        512_000,
    )
    .unwrap();
    let source = source();
    for (path, allowed) in [
        ("/docs/book.pdf", false),
        ("/docs/book.pdf/more", true),
        ("/docs/book.pdf/public", true),
        ("/%C3%A9", false),
        ("/blocked", false),
        ("/docs/%3Asecret", false),
        ("/docs/:secret", true),
    ] {
        assert_eq!(
            rules.allowed("Maestro", &identity(&source, path)),
            allowed,
            "{path}"
        );
    }
    let unicode = Rfc9309::parse("User-agent: *\nDisallow: /é\n".as_bytes(), 512_000).unwrap();
    assert!(!unicode.allowed("Maestro", &identity(&source, "/%C3%A9")));
}

#[test]
fn n10_parser_bounds_and_invalid_rules_refuse() {
    assert!(Rfc9309::parse(&vec![b' '; 512_001], u64::MAX).is_err());
    assert!(Rfc9309::parse(b"User-agent: *\nDisallow: /", 10).is_err());
    for bytes in [
        b"User-agent: *\nDisallow: /%GG".as_slice(),
        b"User-agent: *\nDisallow: nope",
        b"\xff",
    ] {
        assert!(Rfc9309::parse(bytes, 512_000).is_err());
    }
    assert!(Rfc9309::parse(&vec![b' '; 512_000], 512_000).is_ok());
}

#[test]
fn n10_cache_status_ttl_and_scope_fail_closed() {
    let source = source();
    let target = identity(&source, "/docs/page");
    for status in [200, 204, 404, 410] {
        let cache = RobotsCache::response(&target, &source.robots, status, b"", 0);
        assert_eq!(
            cache.check(&target, &source.robots, 999, &DenyOverrides),
            Ok(())
        );
        assert_eq!(
            cache.check(&target, &source.robots, 1000, &DenyOverrides),
            Err(Refusal::Access)
        );
    }
    for status in [301, 401, 403, 429, 500, 503] {
        let cache = RobotsCache::response(&target, &source.robots, status, b"", 0);
        assert_eq!(
            cache.check(&target, &source.robots, 0, &DenyOverrides),
            Err(Refusal::Access)
        );
    }
    let cache = RobotsCache::response(
        &target,
        &source.robots,
        200,
        b"User-agent: *\nDisallow: /docs/private",
        0,
    );
    assert!(
        cache
            .check(
                &identity(&source, "/docs/private"),
                &source.robots,
                0,
                &DenyOverrides
            )
            .is_err()
    );
    let mut other = source.clone();
    other.robots.agent = "Other".into();
    assert!(
        cache
            .check(&target, &other.robots, 0, &DenyOverrides)
            .is_err()
    );
    other
        .origins
        .iter_mut()
        .for_each(|origin| origin.host = "other.example.test".into());
    let other_target =
        FetchIdentity::parse(&other, "https://other.example.test/docs/page").unwrap();
    assert!(
        cache
            .check(&other_target, &source.robots, 0, &DenyOverrides)
            .is_err()
    );
    assert!(
        cache
            .check(&target, &source.robots, 0, &DenyOverrides)
            .is_ok()
    );
    assert!(
        cache
            .check(&target, &source.robots, u64::MAX, &DenyOverrides)
            .is_err()
    );
    let mut long = source.robots.clone();
    long.cache_ttl_ms = u64::MAX.try_into().unwrap();
    let cache = RobotsCache::response(&target, &long, 200, b"", 0);
    assert!(
        cache
            .check(&target, &long, 86_400_000, &DenyOverrides)
            .is_err()
    );
    assert!(
        RobotsCache::unreadable(&target, &source.robots, 0)
            .check(&target, &source.robots, 0, &DenyOverrides)
            .is_err()
    );
}

/// Separately authenticated synthetic authority; never provided by manifest JSON.
#[derive(Debug)]
struct Authority(OverrideDecision);
impl RobotsOverride for Authority {
    fn decide(&self, request: &OverrideRequest<'_>) -> OverrideDecision {
        assert_eq!(request.operation, "robots_override");
        assert_eq!(request.source_id, "notes");
        assert_eq!(request.origin, "https://docs.example.test");
        self.0.clone()
    }
}
#[test]
fn n10_override_missing_expired_wrong_receipt_refuse() {
    let mut source = source();
    let reference = Ref {
        id: "override".into(),
        digest: Digest::of(b"owner receipt"),
    };
    source.robots.r#override = Some(reference.clone());
    let target = identity(&source, "/docs/private");
    let cache = RobotsCache::unreadable(&target, &source.robots, 0);
    for authority in [
        Authority(OverrideDecision::Denied),
        Authority(OverrideDecision::Expired),
        Authority(OverrideDecision::Granted(Ref {
            id: "override".into(),
            digest: Digest::of(b"forged"),
        })),
    ] {
        assert_eq!(
            cache.check(&target, &source.robots, 0, &authority),
            Err(Refusal::Access)
        );
    }
    assert_eq!(
        cache.check(&target, &source.robots, 0, &DenyOverrides),
        Err(Refusal::Access)
    );
    assert_eq!(
        cache.check(
            &target,
            &source.robots,
            0,
            &Authority(OverrideDecision::Granted(reference))
        ),
        Ok(())
    );
}

#[test]
fn n10_restart_resume_and_live_tightening_preserve_interval() {
    let ledger = OriginLedger::new(0);
    let source = source();
    let target = identity(&source, "/docs/page");
    let loose = limits(100, 4);
    assert!(matches!(
        ledger.acquire(&target, loose, Demand::initial("run", 0, 0, 100_000)),
        Err(Pending::Delay { until_ms: 100 })
    ));
    let permit = ledger
        .acquire(&target, loose, Demand::initial("run", 0, 100, 100_000))
        .unwrap();
    drop(permit);
    assert!(matches!(
        ledger.acquire(
            &target,
            limits(1000, 1),
            Demand::initial("run", 0, 200, 100_000)
        ),
        Err(Pending::Delay { until_ms: 1100 })
    ));
    assert!(matches!(
        ledger.acquire(&target, loose, Demand::initial("run", 0, 1000, 100_000)),
        Err(Pending::Delay { until_ms: 1100 })
    ));
    drop(
        ledger
            .acquire(&target, loose, Demand::initial("run", 0, 1100, 100_000))
            .unwrap(),
    );
    let restarted = OriginLedger::new(1100);
    assert!(matches!(
        restarted.acquire(
            &target,
            limits(1000, 1),
            Demand::initial("run", 0, 1100, 100_000)
        ),
        Err(Pending::Delay { until_ms: 2100 })
    ));
}

#[test]
fn n10_http_browser_share_atomic_concurrency_permits() {
    let ledger = OriginLedger::new(0);
    let barrier = Arc::new(Barrier::new(3));
    let acquired = Arc::new(Barrier::new(3));
    let mut handles = Vec::new();
    for _transport in ["http", "browser_render"] {
        let ledger = ledger.clone();
        let barrier = barrier.clone();
        let acquired = acquired.clone();
        handles.push(thread::spawn(move || {
            let source = source();
            barrier.wait();
            let permit = ledger.acquire(
                &identity(&source, "/docs/page"),
                limits(100, 1),
                Demand::initial("run", 0, 100, 100_000),
            );
            acquired.wait();
            permit.is_ok()
        }));
    }
    barrier.wait();
    acquired.wait();
    assert_eq!(
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .filter(|allowed| *allowed)
            .count(),
        1
    );
}

#[test]
fn n10_retry_after_never_shortened_and_retries_finite() {
    let ledger = OriginLedger::new(0);
    let source = source();
    let target = identity(&source, "/docs/page");
    let effective = limits(1000, 1);
    let demand = Demand {
        run_id: "run",
        started_ms: 0,
        now_ms: 1000,
        deadline_ms: 100_000,
        retry: 1,
        server_delay_ms: 60_001,
    };
    assert_eq!(
        pending(ledger.acquire(&target, effective, demand)),
        Pending::Budget
    );
    // Even when this run cannot wait, a second transport cannot bypass the server floor.
    assert_eq!(
        pending(ledger.acquire(&target, effective, Demand::initial("run", 0, 2000, 100_000))),
        Pending::Delay { until_ms: 61_001 }
    );
    assert_eq!(
        pending(ledger.acquire(
            &target,
            effective,
            Demand {
                server_delay_ms: 0,
                retry: 3,
                ..demand
            }
        )),
        Pending::Budget
    );
    assert_eq!(
        pending(ledger.acquire(
            &target,
            effective,
            Demand::initial("run", 0, 61_001, 61_001)
        )),
        Pending::Budget
    );
    drop(
        ledger
            .acquire(
                &target,
                effective,
                Demand::initial("run", 0, 61_001, 100_000),
            )
            .unwrap(),
    );
}

#[test]
fn n10_request_budget_and_concurrency_ceiling_hold() {
    let ledger = OriginLedger::new(0);
    let source = source();
    let target = identity(&source, "/docs/page");
    let mut bounds = source.limits;
    bounds.requests = 1.try_into().unwrap();
    bounds.origin_interval_ms = 100.try_into().unwrap();
    bounds.origin_concurrency = 1.try_into().unwrap();
    let effective = PacingLimits::compose([&bounds]).unwrap();
    let permit = ledger
        .acquire(&target, effective, Demand::initial("run", 0, 100, 100_000))
        .unwrap();
    assert_eq!(
        pending(ledger.acquire(
            &target,
            limits(100, 1),
            Demand::initial("run", 0, 200, 100_000)
        )),
        Pending::Concurrency
    );
    drop(permit);
    assert_eq!(
        pending(ledger.acquire(&target, effective, Demand::initial("run", 0, 200, 100_000))),
        Pending::Budget
    );
}

/// Guard proofs must fail on an assertion, not an unwrap panic.
pub(super) fn pending(result: Result<OriginPermit, Pending>) -> Pending {
    assert!(result.is_err(), "dispatch must remain pending");
    result.unwrap_err()
}
