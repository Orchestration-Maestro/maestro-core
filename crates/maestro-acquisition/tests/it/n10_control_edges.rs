//! Additional actual-engine boundaries and substitute rules adapter.
use super::n10_conform_robots_and_aggregate_origin_pacing::{
    binding, checked_robots, identity, limits, pending, source,
};
use maestro_acquisition::policy::identity::FetchIdentity;
use maestro_acquisition::{
    Refusal,
    transport::{
        pacing::{Demand, OriginLedger, PacingLimits, Pending},
        robots::{DenyOverrides, Rfc9309, RobotsCache, RobotsRules},
    },
};

/// A deliberately disabled substitute result under the same small rules port.
#[derive(Debug)]
struct DisabledRules;
impl RobotsRules for DisabledRules {
    fn allowed(&self, _agent: &str, _identity: &FetchIdentity) -> bool {
        false
    }
}
#[test]
fn n10_substitute_rules_use_same_cache_gate() {
    let source = source();
    let checked = checked_robots(&source.robots);
    let bound = binding(&checked);
    let target = identity(&source, "/docs/page");
    let cache = RobotsCache::parsed(&target, bound, Box::new(DisabledRules), 10, 0);
    assert_eq!(
        cache.check(&target, bound, 0, &DenyOverrides),
        Err(Refusal::Access)
    );
    let cache = RobotsCache::parsed(
        &target,
        bound,
        Box::new(Rfc9309::parse(b"", 512_000).unwrap()),
        20,
        10,
    );
    assert_eq!(cache.check(&target, bound, 10, &DenyOverrides), Ok(()));
    assert_eq!(
        cache.check(&target, bound, 9, &DenyOverrides),
        Err(Refusal::Access)
    );
    let mut unbounded = source.robots.clone();
    unbounded.rules_max_bytes = u64::MAX.try_into().unwrap();
    let oversized = RobotsCache::parsed(
        &target,
        binding(&checked_robots(&unbounded)),
        Box::new(Rfc9309::parse(b"", 512_000).unwrap()),
        512_001,
        10,
    );
    assert_eq!(
        oversized.check(
            &target,
            binding(&checked_robots(&unbounded)),
            10,
            &DenyOverrides
        ),
        Err(Refusal::Access)
    );
    let mut tightened = source.robots.clone();
    tightened.rules_max_bytes = 10.try_into().unwrap();
    assert_eq!(
        cache.check(
            &target,
            binding(&checked_robots(&tightened)),
            10,
            &DenyOverrides
        ),
        Err(Refusal::Access)
    );
    tightened.rules_max_bytes = source.robots.rules_max_bytes;
    tightened.cache_ttl_ms = 10.try_into().unwrap();
    assert_eq!(
        cache.check(
            &target,
            binding(&checked_robots(&tightened)),
            20,
            &DenyOverrides
        ),
        Err(Refusal::Access)
    );
}

#[test]
fn n10_multiple_agents_empty_directives_case_and_anchors() {
    let source = source();
    let rules = Rfc9309::parse(
        b"\xef\xbb\xbfUser-agent: Maestro\n\
        User-agent: Other\n\
        dIsAlLoW: /docs/a*b*c$\n\
        Disallow: /docs/private$\n\
        Allow: /docs/public\n\
        User-agent: Empty\n\
        Disallow:\n\
        User-agent: *\n\
        Disallow: /\n\
        ",
        512_000,
    )
    .unwrap();
    for agent in ["Maestro", "Other"] {
        for (path, allowed) in [
            ("/docs/private", false),
            ("/docs/private/public", true),
            ("/docs/axbyc", false),
            ("/docs/axbyc/more", true),
            ("/docs/abc", false),
            ("/docs/abcbxc", false),
            ("/docs/acb", true),
            ("/docs/public", true),
        ] {
            assert_eq!(
                rules.allowed(agent, &identity(&source, path)),
                allowed,
                "{agent} {path}"
            );
        }
    }
    assert!(rules.allowed("Empty", &identity(&source, "/docs")));
    assert!(!rules.allowed("Bad/Token", &identity(&source, "/docs")));
    assert!(!rules.allowed("", &identity(&source, "/docs")));
    let empty = Rfc9309::parse(b"", 512_000).unwrap();
    assert!(!empty.allowed("Bad/Token", &identity(&source, "/docs")));
    assert!(!empty.allowed("", &identity(&source, "/docs")));
    for bytes in [
        b"User-agent: Bad1\nDisallow: /".as_slice(),
        b"User-agent: *\nDisallow: /%",
        b"User-agent: *\nDisallow: /%0G",
        b"User-agent: *\nDisallow: /\x01",
    ] {
        assert!(Rfc9309::parse(bytes, 512_000).is_err());
    }
    let rules = Rfc9309::parse(
        b"Disallow: /ignored\nUnknown: extension\nUser-agent: *\nDisallow: /docs\n",
        512_000,
    )
    .unwrap();
    assert!(!rules.allowed("Maestro", &identity(&source, "/docs")));
    assert!(rules.allowed("Maestro", &identity(&source, "/ignored")));
}

#[test]
fn n10_run_budgets_span_origins_not_unrelated_runs() {
    let ledger = OriginLedger::new(0);
    let source = source();
    let target = identity(&source, "/docs/page");
    let mut bound = source.limits.clone();
    bound.requests = 1.try_into().unwrap();
    bound.origin_interval_ms = 100.try_into().unwrap();
    let bound = PacingLimits::compose([&bound]).unwrap();
    drop(
        ledger
            .acquire(&target, bound, Demand::initial("one", 0, 100, 100_000))
            .unwrap(),
    );
    let mut other = source.clone();
    other
        .origins
        .iter_mut()
        .for_each(|origin| origin.host = "other.example.test".into());
    let other_target =
        FetchIdentity::parse(&other, "https://other.example.test/docs/page").unwrap();
    assert_eq!(
        pending(ledger.acquire(
            &other_target,
            bound,
            Demand::initial("one", 0, 200, 100_000)
        )),
        Pending::Budget
    );
    drop(
        ledger
            .acquire(
                &other_target,
                bound,
                Demand::initial("two", 0, 200, 100_000),
            )
            .unwrap(),
    );
}

#[test]
fn n10_live_concurrency_tightening_waits_for_owned_work() {
    let ledger = OriginLedger::new(0);
    let source = source();
    let target = identity(&source, "/docs/page");
    let first = ledger
        .acquire(
            &target,
            limits(100, 4),
            Demand::initial("one", 0, 100, 100_000),
        )
        .unwrap();
    let second = ledger
        .acquire(
            &target,
            limits(100, 4),
            Demand::initial("two", 0, 200, 100_000),
        )
        .unwrap();
    assert_eq!(
        pending(ledger.acquire(
            &target,
            limits(100, 1),
            Demand::initial("three", 0, 300, 100_000)
        )),
        Pending::Concurrency
    );
    drop(first);
    assert_eq!(
        pending(ledger.acquire(
            &target,
            limits(100, 4),
            Demand::initial("three", 0, 300, 100_000)
        )),
        Pending::Concurrency
    );
    drop(second);
    drop(
        ledger
            .acquire(
                &target,
                limits(100, 4),
                Demand::initial("three", 0, 300, 100_000),
            )
            .unwrap(),
    );
}

#[test]
fn n10_invalid_time_elapsed_overflow_and_zero_retries_hold() {
    let ledger = OriginLedger::new(0);
    let source = source();
    let target = identity(&source, "/docs/page");
    let effective = limits(100, 1);
    for demand in [
        Demand::initial("", 0, 100, 100_000),
        Demand::initial("before-start", 101, 100, 100_000),
        Demand::initial("overflow", u64::MAX, u64::MAX, u64::MAX),
        Demand {
            server_delay_ms: u64::MAX,
            ..Demand::initial("server-overflow", 0, 100, 100_000)
        },
    ] {
        assert_eq!(
            pending(ledger.acquire(&target, effective, demand)),
            Pending::Budget
        );
    }
    drop(
        ledger
            .acquire(
                &target,
                effective,
                Demand::initial("resume", 0, 100, 100_000),
            )
            .unwrap(),
    );
    for demand in [
        Demand::initial("resume", 1, 200, 100_000),
        Demand::initial("clock-backwards", 0, 99, 100_000),
    ] {
        assert_eq!(
            pending(ledger.acquire(&target, effective, demand)),
            Pending::Budget
        );
    }
    let mut tight = source.limits;
    tight.retries = 0;
    tight.requests = 20.try_into().unwrap();
    tight.origin_interval_ms = 100.try_into().unwrap();
    tight.elapsed_ms = 250.try_into().unwrap();
    let tight = PacingLimits::compose([&tight]).unwrap();
    assert_eq!(
        pending(ledger.acquire(
            &target,
            tight,
            Demand {
                retry: 1,
                ..Demand::initial("resume", 0, 200, 100_000)
            }
        )),
        Pending::Budget
    );
    assert_eq!(
        pending(ledger.acquire(
            &target,
            effective,
            Demand::initial("resume", 0, 250, 100_000)
        )),
        Pending::Budget
    );
    // A floor beyond the elapsed ceiling stays pending rather than being clipped.
    assert_eq!(
        pending(OriginLedger::new(0).acquire(
            &target,
            tight,
            Demand::initial("deadline", 0, 0, 99)
        )),
        Pending::Budget
    );
    assert_eq!(
        pending(OriginLedger::new(u64::MAX).acquire(
            &target,
            effective,
            Demand::initial("interval-overflow", 0, 0, u64::MAX)
        )),
        Pending::Budget
    );
}
