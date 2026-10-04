//! N09 transport guards exercised over real hyper HTTP/1.1.
use super::{n07_parse_url_identity_and_denial_precedence::Controls, n09_support::*};
use maestro_acquisition::{
    Refusal,
    policy::identity::FetchIdentity,
    transport::{connect::OriginCredentials, http::Failure, stream::Accounting},
};
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use std::time::Duration;
use tokio::{task::yield_now, time::Instant};

#[test]
fn n09_identity_stream_and_cross_origin_credentials() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![
            response(
                302,
                "Location: https://other.example/docs/final\r\n",
                b"hop",
            ),
            response(200, "", b"final"),
        ]);
        let identity = FetchIdentity::parse(
            &policy.policy().sources[0],
            "https://garden.example/docs/start",
        )
        .unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer synthetic"));
        let credentials = OriginCredentials::new(&identity, headers);
        let mut request = fetch(identity.as_str());
        request.credentials = Some(&credentials);
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        let result = http(&policy, &controls, &grants, &dns, &wire)
            .fetch(&request, &mut accounting)
            .await
            .unwrap();
        assert_eq!(result.body, b"final");
        assert_eq!(accounting.wire_bytes(), 8);
        assert_eq!(accounting.expanded_bytes(), 8);
        assert_eq!(dns.calls.get(), 2);
        assert_eq!(grants.calls.get(), 2);
        let requests = wire.requests.lock().unwrap();
        assert!(requests[0].contains("Bearer synthetic"));
        assert!(!requests[1].contains("Bearer synthetic"));
        assert_eq!(
            grants.targets.lock().unwrap()[1],
            "https://other.example/docs/final"
        );
    });
}
#[test]
fn n09_redirect_loops_and_policy_denials() {
    run(async {
        for (location, expected) in [
            ("/docs/start", Failure::RedirectLoop),
            ("/docs/private", Failure::Admission(Refusal::Access)),
            (
                "https://unlisted.example/docs/start",
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
            let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
            assert_eq!(
                http(&policy, &controls, &grants, &dns, &wire)
                    .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                    .await
                    .unwrap_err(),
                expected
            );
            assert_eq!(wire.requests.lock().unwrap().len(), 1);
        }
    });
}
#[test]
fn n09_hop_rechecks_dns_and_authority() {
    run(async {
        for denied_dns in [false, true] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            if denied_dns {
                dns.deny_after.set(1);
            } else {
                grants.refuse_after.set(1);
            }
            let wire = Wire::new(vec![response(302, "Location: /docs/next\r\n", b"")]);
            let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
            assert!(
                http(&policy, &controls, &grants, &dns, &wire)
                    .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                    .await
                    .is_err()
            );
            assert_eq!(wire.requests.lock().unwrap().len(), 1);
        }
    });
}
#[test]
fn n09_timeout_drops_owned_connection() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::default();
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        let started = Instant::now();
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Timeout
        );
        // Tokio's timer wheel expires on the next millisecond tick.
        assert!(started.elapsed() >= Duration::from_mins(2));
        assert!(started.elapsed() <= Duration::from_mins(2) + Duration::from_millis(1));
        assert_eq!(*wire.client_dropped.lock().unwrap(), 1);
        yield_now().await;
        assert_eq!(*wire.closed.lock().unwrap(), 1);
    });
}
#[test]
fn n09_encoded_and_expanded_budgets() {
    run(async {
        for (encoded, expected) in [
            (true, Failure::EncodedBytes),
            (false, Failure::ExpandedBytes),
        ] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::new(vec![response(200, "", b"12345")]);
            let mut limits = policy.policy().sources[0].limits.clone();
            if encoded {
                limits.wire_bytes = 4.try_into().unwrap();
            } else {
                limits.decode.expanded_bytes = 4.try_into().unwrap();
            }
            let mut accounting = Accounting::new(limits);
            assert_eq!(
                http(&policy, &controls, &grants, &dns, &wire)
                    .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                    .await
                    .unwrap_err(),
                expected
            );
        }
    });
}
#[test]
fn n09_status_and_content_failures() {
    run(async {
        for (status, headers, body, expected) in [
            (401, "", b"".as_slice(), Failure::Authentication),
            (403, "", b"", Failure::Challenge),
            (206, "", b"", Failure::Partial),
            (200, "Content-Encoding: br\r\n", b"", Failure::Content),
            (
                200,
                "Content-Encoding: gzip\r\n",
                b"invalid",
                Failure::Content,
            ),
        ] {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::new(vec![response(status, headers, body)]);
            let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
            assert_eq!(
                http(&policy, &controls, &grants, &dns, &wire)
                    .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                    .await
                    .unwrap_err(),
                expected
            );
        }
    });
}
