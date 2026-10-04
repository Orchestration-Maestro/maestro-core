//! Read-side quotas, trailer reservation and async completion neighbours.
use super::{
    n07_parse_url_identity_and_denial_precedence::Controls,
    n09_review_support::{ManualClock, SegmentedWire},
    n09_support::*,
};
use maestro_acquisition::transport::{
    http::{Failure, Http},
    stream::{Accounting, TRAILER_MAX_BYTES},
};
use std::{
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use tokio::time::{Instant, advance};

#[test]
fn n09_raw_wire_quota_prevents_read_ahead() {
    run(async {
        for (contiguous, length, cap, read, failure) in [
            (true, 100_000, 4, 0, Some(Failure::EncodedBytes)),
            (true, 4, 4, 4, None),
            (false, 100_000, 4, 4, Some(Failure::EncodedBytes)),
            (false, 3, 4, 3, None),
        ] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let payload = vec![b'x'; length];
            let (headers, payload) = if contiguous {
                (response(200, "", &payload), Vec::new())
            } else {
                (
                    b"HTTP/1.1 200 Synthetic\r\nConnection: close\r\n\r\n".to_vec(),
                    payload,
                )
            };
            let wire = SegmentedWire::new(vec![(headers, payload, Duration::ZERO)]);
            let client = Http {
                policy: &policy,
                controls: &controls,
                authority: &grants,
                resolver: &dns,
                transport: &wire,
                pacing: &ALLOW_PACING,
                pacing_context: pacing_context(),
            };
            let mut limits = policy.policy().sources.first().unwrap().limits.clone();
            limits.wire_bytes = cap.try_into().unwrap();
            let mut accounting = Accounting::new(limits);
            let result = client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await;
            assert_eq!(
                result.err(),
                failure,
                "contiguous={contiguous}, length={length}"
            );
            assert_eq!(wire.payload_read.load(Ordering::SeqCst), read);
            assert_eq!(accounting.wire_bytes(), read as u64);
        }
    });
}

#[test]
fn n09_chunk_framing_and_trailers_share_wire_quota() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let framed = b"1\r\nx\r\n0\r\nX: y\r\n\r\n";
        for cap in [framed.len() as u64, framed.len() as u64 - 1] {
            let wire = SegmentedWire::new(vec![(
                b"HTTP/1.1 200 Synthetic\r\nTransfer-Encoding: chunked\r\n\r\n".to_vec(),
                framed.to_vec(),
                Duration::ZERO,
            )]);
            let client = Http {
                policy: &policy,
                controls: &controls,
                authority: &grants,
                resolver: &dns,
                transport: &wire,
                pacing: &ALLOW_PACING,
                pacing_context: pacing_context(),
            };
            let mut limits = policy.policy().sources.first().unwrap().limits.clone();
            limits.wire_bytes = cap.try_into().unwrap();
            let mut accounting = Accounting::new(limits);
            let result = client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await;
            if cap == framed.len() as u64 {
                let result = result.unwrap();
                assert_eq!(result.body, b"x");
                assert_eq!(result.wire_body, b"x");
                assert!(!result.headers.contains_key("X"));
            } else {
                assert_eq!(result.unwrap_err(), Failure::EncodedBytes);
            }
            assert_eq!(accounting.wire_bytes(), cap);
            assert_eq!(wire.payload_read.load(Ordering::SeqCst) as u64, cap);
        }
    });
}

#[test]
fn n09_trailer_ceiling_and_upfront_reservation_are_explicit() {
    run(async {
        assert_eq!(TRAILER_MAX_BYTES, 16 * 1024);
        for value_bytes in [12_000, usize::try_from(TRAILER_MAX_BYTES).unwrap() + 1] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let raw = format!(
                "HTTP/1.1 200 Synthetic\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nX: {}\r\n\r\n",
                "x".repeat(value_bytes)
            )
            .into_bytes();
            let wire = Wire::new(vec![raw]);
            let mut accounting =
                Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
            let result = http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await;
            assert_eq!(result.is_ok(), value_bytes == 12_000);
        }
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", b"")]);
        let mut limits = policy.policy().sources.first().unwrap().limits.clone();
        limits.memory_bytes = (32_768 + TRAILER_MAX_BYTES - 1).try_into().unwrap();
        let mut accounting = Accounting::new(limits);
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Memory
        );
        assert_eq!(
            wire.responses.lock().unwrap().len(),
            1,
            "reserve before dial"
        );
    });
}

#[test]
fn n09_async_empty_completion_and_unread_robots_obey_trusted_cutoff() {
    run(async {
        for robots in [false, true] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let clock = Arc::new(ManualClock(Mutex::new(Instant::now().into_std())));
            let mut wire = SegmentedWire::new(vec![(
                format!(
                    "HTTP/1.1 {} Synthetic\r\nConnection: close\r\n\r\n",
                    if robots { 404 } else { 200 }
                )
                .into_bytes(),
                Vec::new(),
                Duration::ZERO,
            )]);
            if robots {
                wire.clock_on_headers = Some(clock.clone());
            } else {
                wire.clock_on_eof = Some(clock.clone());
            }
            let client = Http {
                policy: &policy,
                controls: &controls,
                authority: &grants,
                resolver: &dns,
                transport: &wire,
                pacing: &ALLOW_PACING,
                pacing_context: pacing_context(),
            };
            let mut accounting = Accounting::with_clock(
                policy.policy().sources.first().unwrap().limits.clone(),
                clock,
            );
            let mut request = fetch(if robots {
                "https://garden.example/robots.txt"
            } else {
                "https://garden.example/docs/start"
            });
            request.robots = robots;
            assert_eq!(
                client.fetch(&request, &mut accounting).await.unwrap_err(),
                Failure::Timeout
            );
            assert_eq!(accounting.wire_bytes(), 0);
        }
    });
}

#[test]
fn n09_looser_elapsed_limits_never_extend_the_anchored_deadline() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", b""), response(200, "", b"")]);
        let mut limits = policy.policy().sources.first().unwrap().limits.clone();
        limits.elapsed_ms = 1000.try_into().unwrap();
        limits.decode.elapsed_ms = 1000.try_into().unwrap();
        let mut accounting = Accounting::new(limits);
        http(&policy, &controls, &grants, &dns, &wire)
            .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
            .await
            .unwrap();
        advance(Duration::from_secs(1)).await;
        accounting.tighten(&policy.policy().sources.first().unwrap().limits);
        assert_eq!(accounting.limits().elapsed_ms.get(), 1000);
        assert_eq!(accounting.limits().decode.elapsed_ms.get(), 1000);
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Timeout
        );
        assert_eq!(wire.responses.lock().unwrap().len(), 1);
    });
}
