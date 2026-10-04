//! Independent review probes: failures confirm suspected N09 regressions.
use super::{
    n07_parse_url_identity_and_denial_precedence::{self as n07, Controls},
    n09_review_support::*,
    n09_support::*,
    support,
};
use maestro_acquisition::{
    Refusal,
    policy::{
        decision::{RequestKind, admit},
        identity::FetchIdentity,
    },
    transport::{
        http::{Failure, Http},
        robots::{DenyOverrides, RobotsBinding},
        stream::Accounting,
    },
};
use std::{
    cell::RefCell,
    sync::{Arc, Mutex, atomic::Ordering},
    time::{Duration, UNIX_EPOCH},
};
use tokio::time::{Instant, advance};

#[test]
fn n09_partial_robots_disallows_all() {
    run(async {
        let policy = policy();
        let source = policy.policy().sources.first().unwrap();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let partial = response(
            206,
            "Content-Range: bytes 0-0/100\r\n",
            b"User-agent: *\nAllow: /\n",
        );
        let wire = Wire::new(vec![partial.clone(), partial]);
        let transport = http(&policy, &controls, &grants, &dns, &wire);
        let mut request = fetch("https://garden.example/robots.txt");
        request.robots = true;
        let mut accounting = Accounting::new(source.limits.clone());
        let cache = transport
            .fetch_robots(&request, &mut accounting, 0)
            .await
            .unwrap();
        let target = FetchIdentity::parse(source, "https://garden.example/docs/start").unwrap();
        let check = cache.check(
            &target,
            RobotsBinding::new(&policy, "notes").unwrap(),
            0,
            &DenyOverrides,
        );
        let mut direct_accounting = Accounting::new(source.limits.clone());
        let direct = transport.fetch(&request, &mut direct_accounting).await;
        assert!(
            check.is_err(),
            "a partial robots response must disallow content"
        );
        assert_eq!(
            direct.err(),
            Some(Failure::Partial),
            "direct robots fetch must refuse partial output"
        );
    });
}

#[test]
fn n09_tightened_elapsed_is_from_document_start() {
    run(async {
        let policy = policy();
        let source = policy.policy().sources.first().unwrap();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![
            response(200, "", b"first"),
            response(200, "", b"second"),
        ]);
        let transport = http(&policy, &controls, &grants, &dns, &wire);
        let request = fetch("https://garden.example/docs/start");
        let mut accounting = Accounting::new(source.limits.clone());
        assert_eq!(accounting.limits().elapsed_ms.get(), 120_000);
        assert_eq!(accounting.limits().decode.elapsed_ms.get(), 120_000);
        let first = transport.fetch(&request, &mut accounting).await.unwrap();
        assert_eq!(first.body, b"first");
        advance(Duration::from_millis(60_000)).await;
        let mut tightened = accounting.limits().clone();
        tightened.elapsed_ms = 30_000.try_into().unwrap();
        tightened.decode.elapsed_ms = 30_000.try_into().unwrap();
        accounting.tighten(&tightened);
        let second = transport.fetch(&request, &mut accounting).await;
        let dials = 2 - wire.responses.lock().unwrap().len();
        assert_eq!(
            second.err(),
            Some(Failure::Timeout),
            "tightened deadline starts at the original document start"
        );
        assert_eq!(
            grants.calls.get(),
            1,
            "expired accounting must refuse before authority"
        );
        assert_eq!(
            dials, 1,
            "expired accounting must refuse before another dial"
        );
    });
}

#[test]
fn n09_tightened_wire_is_checked_on_empty_response() {
    run(async {
        let policy = policy();
        let source = policy.policy().sources.first().unwrap();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", b"12345"), response(200, "", b"")]);
        let transport = http(&policy, &controls, &grants, &dns, &wire);
        let request = fetch("https://garden.example/docs/start");
        let mut accounting = Accounting::new(source.limits.clone());
        let first = transport.fetch(&request, &mut accounting).await.unwrap();
        assert_eq!(first.body, b"12345");
        assert_eq!(accounting.wire_bytes(), 5);
        let mut tightened = accounting.limits().clone();
        tightened.wire_bytes = 4.try_into().unwrap();
        accounting.tighten(&tightened);
        let second = transport.fetch(&request, &mut accounting).await;
        assert_eq!(
            second.err(),
            Some(Failure::EncodedBytes),
            "an empty response must still enforce the tightened cumulative wire ceiling"
        );
        assert_eq!(
            accounting.wire_bytes(),
            5,
            "previous wire accounting is retained"
        );
        assert_eq!(
            accounting.expanded_bytes(),
            5,
            "previous decode accounting is retained"
        );
        assert_eq!(grants.calls.get(), 1);
        assert_eq!(dns.calls.get(), 1);
        assert_eq!(wire.responses.lock().unwrap().len(), 1);
    });
}

#[test]
fn n09_oversized_body_refuses_before_read() {
    run(async {
        let policy = policy();
        let source = policy.policy().sources.first().unwrap();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        // Check the allowed neighbour before the rejecting probe can fail.
        let mut oversized = None;
        for length in [4, 100_000] {
            let payload = if length == 4 {
                b"1234".to_vec()
            } else {
                vec![b'x'; length]
            };
            let wire = SegmentedWire::new(vec![(
                format!("HTTP/1.1 200 Synthetic\r\nContent-Length: {length}\r\n\r\n").into_bytes(),
                payload,
                Duration::ZERO,
            )]);
            let mut limits = source.limits.clone();
            limits.wire_bytes = 4.try_into().unwrap();
            let mut accounting = Accounting::new(limits);
            let client = Http {
                policy: &policy,
                controls: &controls,
                authority: &grants,
                resolver: &dns,
                transport: &wire,
                pacing: &ALLOW_PACING,
                pacing_context: pacing_context(),
            };
            let result = client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await;
            let payload_read = wire.payload_read.load(Ordering::SeqCst);
            if length == 4 {
                assert_eq!(result.unwrap().body, b"1234");
                assert_eq!(payload_read, 4);
            } else {
                oversized = Some((result.err(), payload_read));
            }
        }
        assert_eq!(
            oversized,
            Some((Some(Failure::EncodedBytes), 0)),
            "known oversized bodies must refuse before any payload read"
        );
    });
}

#[test]
fn n09_policy_time_is_fresh_per_redirect() {
    run(async {
        let (mut collection, mut catalog) = n07::fixture();
        let mut decisions = support::value(&catalog, "decisions");
        decisions["entries"][0]["effective_at"] = "2026-09-30T20:00:01Z".into();
        support::put(&mut catalog, "decisions", &decisions);
        let generous = policy();
        let limits =
            serde_json::to_value(&generous.policy().sources.first().unwrap().limits).unwrap();
        let mut policy_wire = support::value(&catalog, "policy");
        policy_wire["sources"][0]["limits"] = limits.clone();
        policy_wire["aggregate_limits"] = limits;
        support::put(&mut catalog, "policy", &policy_wire);
        support::rebind(&mut collection, &mut catalog);
        let policy = n07::checked(collection, &catalog).unwrap();
        let controls = TimedControls {
            inner: Controls::default(),
            started: Instant::now(),
            stages: RefCell::new(Vec::new()),
        };
        let grants = TimedGrants::default();
        let dns = Dns::default();
        let wire = SegmentedWire::new(vec![
            (
                response(302, "Location: /docs/private\r\n", b""),
                vec![],
                Duration::from_secs(2),
            ),
            (response(200, "", b""), vec![], Duration::ZERO),
        ]);
        let mut request = fetch("https://garden.example/docs/start");
        request.authority_time = UNIX_EPOCH + Duration::from_hours(497_444);
        assert_eq!(request.request.now, "2026-09-30T20:00:00Z");
        assert!(admit(&policy, &request.request, &Controls::default()).is_ok());
        let mut current_private =
            n07::request("https://garden.example/docs/private", RequestKind::Redirect);
        current_private.now = "2026-09-30T20:00:02Z";
        assert_eq!(
            admit(&policy, &current_private, &Controls::default()).unwrap_err(),
            Refusal::Access
        );
        let mut accounting =
            Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
        let client = Http {
            policy: &policy,
            controls: &controls,
            authority: &grants,
            resolver: &dns,
            transport: &wire,
            pacing: &ALLOW_PACING,
            pacing_context: pacing_context(),
        };
        let result = client.fetch(&request, &mut accounting).await;
        let authority_elapsed_ms = grants
            .times
            .borrow()
            .iter()
            .map(|time| {
                time.duration_since(request.authority_time)
                    .unwrap()
                    .as_millis()
            })
            .collect::<Vec<_>>();
        assert_eq!(authority_elapsed_ms, vec![0]);
        assert_eq!(
            result.err(),
            Some(Failure::Admission(Refusal::Access)),
            "the redirect must apply the now-effective deny"
        );
        assert_eq!(wire.dials.get(), 1, "denied redirect must not dial");
    });
}

#[test]
fn n09_trailers_are_charged_and_discarded() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let raw = format!(
            "HTTP/1.1 200 Synthetic\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nX-Long: {}\r\n\r\n",
            "x".repeat(12_000)
        )
        .into_bytes();
        let framed_bytes = raw.len()
            - raw
                .windows(4)
                .position(|bytes| bytes == b"\r\n\r\n")
                .unwrap()
            - 4;
        let wire = Wire::new(vec![raw]);
        let mut accounting =
            Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
        let result = http(&policy, &controls, &grants, &dns, &wire)
            .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
            .await;
        let result = result.unwrap();
        assert!(result.body.is_empty());
        assert!(result.wire_body.is_empty());
        assert!(result.headers.get("X-Long").is_none());
        assert_eq!(accounting.expanded_bytes(), 0);
        assert_eq!(
            accounting.wire_bytes(),
            framed_bytes as u64,
            "raw post-header bytes include chunk framing and trailers"
        );
    });
}

#[test]
fn n09_async_connect_cutoff_empty_response_refuses() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", b"")]);
        let started = Instant::now().into_std();
        let clock = Arc::new(ManualClock(Mutex::new(started)));
        let transport = CutoffWire {
            wire: &wire,
            clock: clock.clone(),
            elapsed: Duration::from_mins(2),
        };
        let mut accounting = Accounting::with_clock(
            policy.policy().sources.first().unwrap().limits.clone(),
            clock.clone(),
        );
        let client = Http {
            policy: &policy,
            controls: &controls,
            authority: &grants,
            resolver: &dns,
            transport: &transport,
            pacing: &ALLOW_PACING,
            pacing_context: pacing_context(),
        };
        let result = client
            .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
            .await;
        let requests = wire.requests.lock().unwrap().clone();
        let elapsed_ms = clock.0.lock().unwrap().duration_since(started).as_millis();
        assert_eq!(elapsed_ms, 120_000);
        assert_eq!(
            result.err(),
            Some(Failure::Timeout),
            "async connection completion at the trusted cutoff must refuse"
        );
        assert!(
            requests.is_empty(),
            "no HTTP request may be emitted after the cutoff"
        );
    });
}
