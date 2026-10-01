//! Parser allocation floors and hostile header/status lines stay bounded.
use super::{n07_parse_url_identity_and_denial_precedence::Controls, n09_support::*};
use maestro_acquisition::transport::{http::Failure, stream::Accounting};
use reqwest::header::{CONTENT_TYPE, LOCATION, SET_COOKIE};
use std::fmt::Write as _;

#[test]
fn n09_oversized_header_and_status_line_refuse() {
    run(async {
        for bytes in [
            response(
                200,
                &format!("X-Synthetic: {}\r\n", "x".repeat(50_000)),
                b"",
            ),
            format!(
                "HTTP/1.1 200 {}\r\nContent-Length: 0\r\n\r\n",
                "x".repeat(50_000)
            )
            .into_bytes(),
        ] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::new(vec![bytes]);
            let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
            assert_eq!(
                http(&policy, &controls, &grants, &dns, &wire)
                    .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                    .await
                    .unwrap_err(),
                Failure::Content
            );
            assert_eq!(accounting.wire_bytes(), 0);
        }
    });
}
#[test]
fn n09_parser_workspace_refuses_before_connection() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", b"")]);
        let mut limits = policy.policy().sources[0].limits.clone();
        limits.memory_bytes = 8191.try_into().unwrap();
        let mut accounting = Accounting::new(limits);
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Memory
        );
        assert!(wire.requests.lock().unwrap().is_empty());
    });
}

#[test]
fn n09_header_count_obeys_memory_derived_ceiling() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let headers = (0..250).fold(String::new(), |mut headers, n| {
            write!(headers, "X-{n}: x\r\n").unwrap();
            headers
        });
        assert!(headers.len() < 8192);
        let wire = Wire::new(vec![response(200, &headers, b"")]);
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
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
fn n09_transient_location_and_cookies_are_not_safe_metadata() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(
            200,
            concat!(
                "Location: https://garden.example/docs/start?token=synthetic-secret\r\n",
                "Set-Cookie: synthetic-secret\r\n",
                "Content-Type: text/plain\r\n"
            ),
            b"ok",
        )]);
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        let result = http(&policy, &controls, &grants, &dns, &wire)
            .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
            .await
            .unwrap();
        assert!(!result.headers.contains_key(LOCATION));
        assert!(!result.headers.contains_key(SET_COOKIE));
        assert_eq!(result.headers.get(CONTENT_TYPE).unwrap(), "text/plain");
        assert!(!format!("{result:?}").contains("synthetic-secret"));
    });
}

#[test]
fn n09_parser_workspace_remains_charged_while_reading() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", b"ok")]);
        let mut limits = policy.policy().sources[0].limits.clone();
        limits.memory_bytes = 32_768.try_into().unwrap();
        let mut accounting = Accounting::new(limits);
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Memory
        );
    });
}
