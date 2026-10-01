//! The independent logical-run cutoff constrains still-live document work.
use super::{
    n07_parse_url_identity_and_denial_precedence::Controls,
    n09_review_support::{CutoffWire, ManualClock, SegmentedWire},
    n09_support::*,
};
use maestro_acquisition::transport::{
    http::{Failure, Http},
    stream::Accounting,
};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::time::{Instant, advance};

#[test]
fn n09_run_deadline_expiring_in_connect_prevents_request() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", b"")]);
        let start = Instant::now().into_std();
        let clock = Arc::new(ManualClock(Mutex::new(start)));
        let transport = CutoffWire {
            wire: &wire,
            clock: clock.clone(),
            elapsed: Duration::from_secs(1),
        };
        let mut context = pacing_context();
        context.deadline_ms = 1000;
        let client = Http {
            policy: &policy,
            controls: &controls,
            authority: &grants,
            resolver: &dns,
            transport: &transport,
            pacing: &ALLOW_PACING,
            pacing_context: context,
        };
        let mut accounting = Accounting::with_clock(
            policy.policy().sources.first().unwrap().limits.clone(),
            clock.clone(),
        );
        assert_eq!(
            client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Timeout
        );
        assert_eq!(
            clock.0.lock().unwrap().duration_since(start),
            Duration::from_secs(1)
        );
        assert!(
            accounting.limits().elapsed_ms.get() > 1000,
            "document remains live"
        );
        assert!(
            wire.requests.lock().unwrap().is_empty(),
            "no request at the run cutoff"
        );
    });
}

#[test]
fn n09_run_deadline_expiring_in_body_prevents_success_or_retry() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let start = Instant::now().into_std();
        let clock = Arc::new(ManualClock(Mutex::new(start)));
        let mut wire = SegmentedWire::new(vec![(
            b"HTTP/1.1 200 Synthetic\r\nConnection: close\r\n\r\n".to_vec(),
            b"x".to_vec(),
            Duration::ZERO,
        )]);
        wire.clock_on_eof = Some(clock.clone());
        wire.completion_elapsed = Duration::from_secs(1);
        let mut context = pacing_context();
        context.deadline_ms = 1000;
        let client = Http {
            policy: &policy,
            controls: &controls,
            authority: &grants,
            resolver: &dns,
            transport: &wire,
            pacing: &ALLOW_PACING,
            pacing_context: context,
        };
        let mut accounting = Accounting::with_clock(
            policy.policy().sources.first().unwrap().limits.clone(),
            clock.clone(),
        );
        assert_eq!(
            client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Timeout
        );
        assert_eq!(
            clock.0.lock().unwrap().duration_since(start),
            Duration::from_secs(1)
        );
        assert!(accounting.limits().elapsed_ms.get() > 1000);
        // The segmented server waits for the first request before returning any
        // body, so its sole dispatch precedes EOF's clock advance. No late retry.
        assert_eq!(wire.dials.get(), 1);
        assert_eq!(accounting.wire_bytes(), 1);
    });
}

#[test]
fn n09_looser_run_deadline_never_extends_owned_cutoff() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", b""), response(200, "", b"")]);
        let mut context = pacing_context();
        context.deadline_ms = 1000;
        let client = Http {
            policy: &policy,
            controls: &controls,
            authority: &grants,
            resolver: &dns,
            transport: &wire,
            pacing: &ALLOW_PACING,
            pacing_context: context,
        };
        let mut accounting =
            Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
        client
            .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
            .await
            .unwrap();
        advance(Duration::from_secs(1)).await;
        let later = Http {
            pacing_context: maestro_acquisition::transport::pacing::PacingContext {
                deadline_ms: 5000,
                ..context
            },
            ..client
        };
        assert_eq!(
            later
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Timeout
        );
        assert_eq!(wire.responses.lock().unwrap().len(), 1);
        assert_eq!(grants.calls.get(), 1);
    });
}

#[test]
fn n09_tightened_decode_deadline_uses_original_document_start() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![
            response(200, "", b"first"),
            response(200, "", b"second"),
        ]);
        let client = http(&policy, &controls, &grants, &dns, &wire);
        let mut accounting =
            Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
        client
            .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
            .await
            .unwrap();
        advance(Duration::from_mins(1)).await;
        let mut tightened = accounting.limits().clone();
        tightened.decode.elapsed_ms = 30_000.try_into().unwrap();
        accounting.tighten(&tightened);
        // The independent run deadline is still live: only the document decode
        // anchor can refuse this tightened envelope before further effects.
        assert_eq!(accounting.limits().elapsed_ms.get(), 120_000);
        assert_eq!(
            client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Timeout
        );
        assert_eq!(wire.responses.lock().unwrap().len(), 1);
        assert_eq!(grants.calls.get(), 1);
    });
}

#[test]
fn n09_run_elapsed_ceiling_bounds_already_running_work() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", b"")]);
        let mut context = pacing_context();
        context.deadline_ms = 500_000;
        advance(Duration::from_secs(119)).await;
        let clock = Arc::new(ManualClock(Mutex::new(Instant::now().into_std())));
        let transport = CutoffWire {
            wire: &wire,
            clock: clock.clone(),
            elapsed: Duration::from_secs(1),
        };
        let client = Http {
            policy: &policy,
            controls: &controls,
            authority: &grants,
            resolver: &dns,
            transport: &transport,
            pacing: &ALLOW_PACING,
            pacing_context: context,
        };
        let mut accounting = Accounting::with_clock(
            policy.policy().sources.first().unwrap().limits.clone(),
            clock,
        );
        assert_eq!(
            client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Timeout
        );
        assert!(wire.requests.lock().unwrap().is_empty());
    });
}
