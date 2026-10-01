//! Informational blocks share finite header bounds without body read-ahead.
use super::{
    n07_parse_url_identity_and_denial_precedence::Controls,
    n09_review_support::{ManualClock, SegmentedWire},
    n09_support::*,
};
use maestro_acquisition::transport::{
    http::{Failure, Http, MAX_INTERIM_RESPONSES},
    stream::Accounting,
};
use reqwest::header::{HeaderName, HeaderValue};
use std::{
    fmt::Write as _,
    sync::{Arc, Mutex, atomic::Ordering},
    time::Duration,
};
use tokio::time::Instant;

/// Synthetic status plus independently chosen header fields or padding.
fn block(status: u16, fields: usize, padding: usize) -> Vec<u8> {
    let mut raw = format!("HTTP/1.1 {status} Synthetic\r\n");
    for field in 0..fields {
        write!(raw, "X-{field}: synthetic\r\n").unwrap();
    }
    if padding != 0 {
        write!(raw, "X-Padding: {}\r\n", "x".repeat(padding)).unwrap();
    }
    raw.push_str("\r\n");
    raw.into_bytes()
}
/// The final response stays within its own byte/count limits in every case.
fn check(mut prefix: Vec<u8>, expected: &Result<u16, Failure>) {
    prefix.extend(response(200, "", b"ok"));
    let success = expected.is_ok();
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![prefix]);
        let mut accounting =
            Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
        let result = http(&policy, &controls, &grants, &dns, &wire)
            .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
            .await;
        let result = result.map(|response| {
            if success {
                assert_eq!(response.body, b"ok");
                assert_eq!(response.wire_body, b"ok");
                assert_eq!(
                    accounting.wire_bytes(),
                    2,
                    "informational headers are header bytes, not DATA"
                );
            }
            response.status
        });
        assert_eq!(&result, expected);
    });
}

#[test]
fn n09_103_early_hints_then_final_response_succeeds() {
    check(block(103, 1, 0), &Ok(200));
}
#[test]
fn n09_100_continue_then_final_response_succeeds() {
    check(block(100, 0, 0), &Ok(200));
}
#[test]
fn n09_several_interim_responses_within_limits_succeed() {
    let mut prefix = Vec::new();
    for status in [100, 102, 103, 199] {
        prefix.extend(block(status, 1, 0));
    }
    check(prefix, &Ok(200));
}
#[test]
fn n09_101_protocol_switch_refuses() {
    check(block(101, 0, 0), &Err(Failure::Content));
}
#[test]
fn n09_interim_response_cap_refuses_without_final_promotion() {
    assert_eq!(MAX_INTERIM_RESPONSES, 8);
    let mut prefix = Vec::new();
    for _ in 0..=MAX_INTERIM_RESPONSES {
        prefix.extend(block(103, 0, 0));
    }
    check(prefix, &Err(Failure::Content));
}
#[test]
fn n09_interim_header_bytes_are_charged_cumulatively() {
    let mut prefix = block(103, 0, 5000);
    prefix.extend(block(103, 0, 5000));
    check(prefix, &Err(Failure::Content));
}
#[test]
fn n09_interim_and_final_header_fields_share_count_limit() {
    // Each block stays under hyper's individual 100-slot limit; their combined
    // field count exceeds the memory-derived cumulative count ceiling.
    assert!(8192 / size_of::<(HeaderName, HeaderValue)>() < 180);
    let mut prefix = block(103, 90, 0);
    let mut final_block = block(200, 90, 0);
    // A final response without Content-Length ends on the server's owned EOF.
    prefix.append(&mut final_block);
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![prefix]);
        let mut accounting =
            Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Content
        );
    });
}
#[test]
fn n09_hyper_default_header_slots_remain_a_fixed_ceiling() {
    assert!(8192 / size_of::<(HeaderName, HeaderValue)>() > 100);
    check(block(200, 101, 0), &Err(Failure::Content));
}
#[test]
fn n09_interim_at_trusted_deadline_makes_no_further_read() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let clock = Arc::new(ManualClock(Mutex::new(Instant::now().into_std())));
        let mut raw = block(103, 0, 0);
        raw.extend(response(200, "", b"ok"));
        let mut wire = SegmentedWire::new(vec![(raw, Vec::new(), Duration::ZERO)]);
        wire.clock_on_headers = Some(clock.clone());
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
        assert_eq!(
            client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Timeout
        );
        assert_eq!(
            wire.payload_read.load(Ordering::SeqCst),
            0,
            "no next-header read after cutoff"
        );
        assert_eq!(wire.dials.get(), 1);
    });
}
