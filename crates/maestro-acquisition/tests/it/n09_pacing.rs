//! HTTP owns pacing for first hops, redirects, retries and robots refreshes.
use super::{
    n07_parse_url_identity_and_denial_precedence::Controls, n09_pacing_support::RecordingLedger,
    n09_support::*,
};
use maestro_acquisition::policy::identity::FetchIdentity;
use maestro_acquisition::transport::{
    http::Http,
    pacing::{Demand, PacingLimits, Pending},
    stream::Accounting,
};
use std::time::{Duration, UNIX_EPOCH};
use tokio::{
    task::yield_now,
    time::{Instant, advance},
};

#[test]
fn n09_pacing_reservations_match_all_owned_dispatches() {
    run(async {
        let policy = policy_with(|wire| {
            wire["sources"][0]["limits"]["max_backoff_ms"] = 5000.into();
            wire["aggregate_limits"] = wire["sources"][0]["limits"].clone();
        });
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let ledger = RecordingLedger::new();
        let wire = Wire::new(vec![
            response(302, "Location: /docs/next\r\n", b""),
            response(302, "Location: https://other.example/docs/final\r\n", b""),
            response(200, "", b"ok"),
            response(404, "", b""),
            response(410, "", b""),
        ]);
        wire.failures.set(1);
        let client = Http {
            policy: &policy,
            controls: &controls,
            authority: &grants,
            resolver: &dns,
            transport: &wire,
            pacing: &ledger,
            pacing_context: pacing_context(),
        };
        let source = policy.policy().sources.first().unwrap();
        let mut accounting = Accounting::new(source.limits.clone());
        assert_eq!(
            client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap()
                .body,
            b"ok"
        );
        for _ in 0..2 {
            let mut accounting = Accounting::new(source.limits.clone());
            client
                .fetch_robots(
                    &fetch("https://garden.example/robots.txt"),
                    &mut accounting,
                    0,
                )
                .await
                .unwrap();
        }
        let observed = ledger.observations.lock().unwrap();
        assert_eq!(
            observed.acquired.len(),
            6,
            "one failed connect plus five HTTP dispatches"
        );
        assert_eq!(observed.released, 6);
        assert_eq!(observed.active, 0);
        assert_eq!(observed.acquired.get(1).unwrap().1, 1, "retry is paced too");
        assert_eq!(observed.acquired.get(3).unwrap().0, "https://other.example");
        assert_eq!(wire.requests.lock().unwrap().len(), 5);
    });
}

#[test]
fn n09_pacing_denial_prevents_first_and_robots_dials() {
    run(async {
        for robots in [false, true] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let mut ledger = RecordingLedger::new();
            ledger.deny = true;
            let wire = Wire::new(vec![response(200, "", b"")]);
            let client = Http {
                policy: &policy,
                controls: &controls,
                authority: &grants,
                resolver: &dns,
                transport: &wire,
                pacing: &ledger,
                pacing_context: pacing_context(),
            };
            let mut request = fetch(if robots {
                "https://garden.example/robots.txt"
            } else {
                "https://garden.example/docs/start"
            });
            request.robots = robots;
            let mut accounting =
                Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
            assert!(client.fetch(&request, &mut accounting).await.is_err());
            assert_eq!(
                wire.responses.lock().unwrap().len(),
                1,
                "denial means zero dials"
            );
            assert!(wire.requests.lock().unwrap().is_empty());
        }
    });
}

#[test]
fn n09_pacing_shared_concurrency_prevents_dial() {
    run(async {
        let policy = policy();
        let source = policy.policy().sources.first().unwrap();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let ledger = RecordingLedger::new();
        let wire = Wire::new(vec![response(200, "", b"")]);
        let context = pacing_context();
        let limits = PacingLimits::compose([&source.limits]).unwrap();
        advance(Duration::from_millis(limits.interval_ms())).await;
        let identity = FetchIdentity::parse(source, "https://garden.example/docs/start").unwrap();
        let now = u64::try_from(
            Instant::now()
                .into_std()
                .duration_since(context.epoch)
                .as_millis(),
        )
        .unwrap();
        let held = ledger
            .inner
            .acquire(
                &identity,
                limits,
                Demand::initial("browser", 0, now, 120_000),
            )
            .unwrap();
        let client = Http {
            policy: &policy,
            controls: &controls,
            authority: &grants,
            resolver: &dns,
            transport: &wire,
            pacing: &ledger,
            pacing_context: context,
        };
        let mut accounting = Accounting::new(source.limits.clone());
        assert!(
            client
                .fetch(&fetch(identity.as_str()), &mut accounting)
                .await
                .is_err()
        );
        assert_eq!(wire.responses.lock().unwrap().len(), 1);
        drop(held);
    });
}

#[test]
fn n09_pacing_failure_backoff_is_never_clamped() {
    run(async {
        let policy = policy_with(|wire| {
            wire["sources"][0]["limits"]["origin_interval_ms"] = 1000.into();
            wire["sources"][0]["limits"]["retries"] = 2.into();
            wire["sources"][0]["limits"]["max_backoff_ms"] = 1000.into();
            wire["aggregate_limits"] = wire["sources"][0]["limits"].clone();
        });
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let ledger = RecordingLedger::new();
        let wire = Wire::default();
        wire.failures.set(3);
        let client = Http {
            policy: &policy,
            controls: &controls,
            authority: &grants,
            resolver: &dns,
            transport: &wire,
            pacing: &ledger,
            pacing_context: pacing_context(),
        };
        let mut accounting =
            Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
        let started = Instant::now();
        assert!(
            client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .is_err()
        );
        assert_eq!(
            wire.failures.get(),
            1,
            "second retry needs 2000ms and must not dial"
        );
        assert_eq!(ledger.observations.lock().unwrap().released, 2);
        assert!(started.elapsed() >= Duration::from_secs(2));
    });
}

#[test]
fn n09_retry_after_floor_and_malformed_values_are_enforced() {
    run(async {
        for (value, minimum) in [
            ("3", Some(3000)),
            ("Wed, 30 Sep 2026 20:00:03 GMT", Some(1999)),
            ("Wed, 30 Sep 2026 19:00:00 GMT", Some(1000)),
            ("bad", None),
            ("Wednesday, 30-Sep-26 20:00:03 GMT", None),
            ("Wed Sep 30 20:00:03 2026", None),
        ] {
            let policy = policy_with(|wire| {
                wire["sources"][0]["limits"]["origin_interval_ms"] = 1000.into();
                wire["sources"][0]["limits"]["max_backoff_ms"] = 5000.into();
                wire["aggregate_limits"] = wire["sources"][0]["limits"].clone();
            });
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let ledger = RecordingLedger::new();
            let wire = Wire::new(vec![
                response(503, &format!("Retry-After: {value}\r\n"), b""),
                response(200, "", b"ok"),
            ]);
            let client = Http {
                policy: &policy,
                controls: &controls,
                authority: &grants,
                resolver: &dns,
                transport: &wire,
                pacing: &ledger,
                pacing_context: pacing_context(),
            };
            let mut request = fetch("https://garden.example/docs/start");
            request.authority_time = UNIX_EPOCH + Duration::from_hours(497_444);
            let mut accounting =
                Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
            let result = client.fetch(&request, &mut accounting).await;
            let observed = ledger.observations.lock().unwrap();
            if let Some(minimum) = minimum {
                assert_eq!(result.unwrap().status, 200);
                assert_eq!(observed.acquired.len(), 2);
                let first = observed.acquired.first().unwrap();
                let retry = observed.acquired.get(1).unwrap();
                assert!(
                    retry.2 - first.2 >= minimum,
                    "server floor {value}: {observed:?}"
                );
            } else {
                assert!(result.is_err(), "unknown delay must refuse retry");
                assert_eq!(wire.requests.lock().unwrap().len(), 1);
            }
        }
    });
}

#[test]
fn n09_pacing_denial_prevents_hidden_redirect_and_retry_dials() {
    run(async {
        for retry in [false, true] {
            let policy = policy_with(|wire| {
                wire["sources"][0]["limits"]["max_backoff_ms"] = 5000.into();
                wire["aggregate_limits"] = wire["sources"][0]["limits"].clone();
            });
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let mut ledger = RecordingLedger::new();
            ledger.deny_after = Some(1);
            let wire = Wire::new(vec![
                response(302, "Location: /docs/next\r\n", b""),
                response(200, "", b""),
            ]);
            wire.failures.set(usize::from(retry));
            let client = Http {
                policy: &policy,
                controls: &controls,
                authority: &grants,
                resolver: &dns,
                transport: &wire,
                pacing: &ledger,
                pacing_context: pacing_context(),
            };
            let mut accounting =
                Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
            assert!(
                client
                    .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                    .await
                    .is_err()
            );
            assert_eq!(
                wire.responses.lock().unwrap().len(),
                if retry { 2 } else { 1 }
            );
            assert_eq!(ledger.observations.lock().unwrap().released, 1);
        }
    });
}

#[test]
fn n09_pacing_permit_is_held_until_owned_work_stops() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let ledger = RecordingLedger::new();
        let wire = Wire::default();
        let source = policy.policy().sources.first().unwrap();
        let context = pacing_context();
        let limits = PacingLimits::compose([&source.limits]).unwrap();
        let client = Http {
            policy: &policy,
            controls: &controls,
            authority: &grants,
            resolver: &dns,
            transport: &wire,
            pacing: &ledger,
            pacing_context: context,
        };
        let mut accounting = Accounting::new(source.limits.clone());
        let request = fetch("https://garden.example/docs/start");
        let identity = FetchIdentity::parse(source, request.request.url).unwrap();
        advance(Duration::from_millis(limits.interval_ms())).await;
        let mut work = Box::pin(client.fetch(&request, &mut accounting));
        let observed = async {
            while wire.requests.lock().unwrap().is_empty() {
                yield_now().await;
            }
            let now = u64::try_from(
                Instant::now()
                    .into_std()
                    .duration_since(context.epoch)
                    .as_millis(),
            )
            .unwrap();
            assert_eq!(
                ledger
                    .inner
                    .acquire(
                        &identity,
                        limits,
                        Demand::initial("browser", 0, now, 120_000)
                    )
                    .unwrap_err(),
                Pending::Concurrency
            );
        };
        tokio::select! { biased;
            () = observed => (),
            result = &mut work => panic!("stalled owned work completed early: {result:?}"),
        }
        drop(work);
        assert_eq!(ledger.observations.lock().unwrap().active, 0);
        assert_eq!(ledger.observations.lock().unwrap().released, 1);
        assert_eq!(*wire.client_dropped.lock().unwrap(), 1);
    });
}
