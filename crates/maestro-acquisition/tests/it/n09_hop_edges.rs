//! Attempt, redirect, authority and relative-reference boundary guards.
use super::{n07_parse_url_identity_and_denial_precedence::Controls, n09_support::*};
use maestro_acquisition::Refusal;
use maestro_acquisition::policy::decision::RequestKind;
use maestro_acquisition::transport::{http::Failure, stream::Accounting};
use serde_json::json;
use std::time::Duration;
use tokio::time::advance;

#[test]
fn n09_revoked_grant_refuses_fresh_retry() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::default();
        wire.failures.set(1);
        grants.refuse_after.set(1);
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        assert!(matches!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await,
            Err(Failure::Authority(_))
        ));
        assert_eq!(grants.calls.get(), 2);
        assert_eq!(dns.calls.get(), 2);
        assert!(wire.requests.lock().unwrap().is_empty());
    });
}
#[test]
fn n09_retry_ceiling_and_attempt_ceiling() {
    run(async {
        for (requests, expected, calls) in [(1, Failure::Requests, 1), (10, Failure::Transport, 2)]
        {
            let policy = policy();
            let controls = Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::default();
            wire.failures.set(10);
            let mut limits = policy.policy().sources[0].limits.clone();
            limits.requests = requests.try_into().unwrap();
            let mut accounting = Accounting::new(limits);
            assert_eq!(
                http(&policy, &controls, &grants, &dns, &wire)
                    .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                    .await
                    .unwrap_err(),
                expected
            );
            assert_eq!(grants.calls.get(), calls);
        }
    });
}
#[test]
fn n09_redirect_limit_refuses_next_dispatch() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![
            response(302, "Location: /docs/second\r\n", b""),
            response(302, "Location: /docs/third\r\n", b""),
        ]);
        let mut limits = policy.policy().sources[0].limits.clone();
        limits.redirects = 1.try_into().unwrap();
        let mut accounting = Accounting::new(limits);
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::RedirectLimit
        );
        assert_eq!(grants.calls.get(), 2);
    });
}
#[test]
fn n09_relative_redirects_preserve_unsafe_raw_paths() {
    run(async {
        for location in ["/docs/x/../start", "x/../start", "/docs/%2e/start"] {
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
            assert!(
                matches!(
                    http(&policy, &controls, &grants, &dns, &wire)
                        .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                        .await,
                    Err(Failure::Admission(_))
                ),
                "unsafe redirect {location}"
            );
        }
    });
}
#[test]
fn n09_expired_document_budget_cannot_restart() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", b"ok")]);
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        let client = http(&policy, &controls, &grants, &dns, &wire);
        assert!(
            client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .is_ok()
        );
        advance(Duration::from_millis(120_001)).await;
        assert_eq!(
            client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Timeout
        );
        assert_eq!(grants.calls.get(), 1);
    });
}

#[test]
fn n09_retry_entry_redirect_loop_refuses_second_dial() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(302, "Location: /docs/start\r\n", b"")]);
        let mut request = fetch("https://garden.example/docs/start");
        request.request.kind = RequestKind::Retry;
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&request, &mut accounting)
                .await
                .unwrap_err(),
            Failure::RedirectLoop
        );
        assert_eq!(grants.calls.get(), 1);
    });
}

#[test]
fn n09_query_is_identity_but_not_authority_target() {
    run(async {
        let policy = policy_with(|wire| {
            wire["sources"][0]["identity"]["meaningful_queries"] = json!(["id"]);
        });
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![
            response(302, "Location: /docs/final?id=two\r\n", b""),
            response(200, "", b"ok"),
        ]);
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        let result = http(&policy, &controls, &grants, &dns, &wire)
            .fetch(
                &fetch("https://garden.example/docs/start?id=one"),
                &mut accounting,
            )
            .await
            .unwrap();
        assert!(result.identity.as_str().ends_with("?id=two"));
        assert_eq!(
            *grants.targets.lock().unwrap(),
            [
                "https://garden.example/docs/start",
                "https://garden.example/docs/final"
            ]
        );
    });
}

#[test]
fn n09_invalid_host_authority_context_refuses_without_dispatch() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::default();
        let mut request = fetch("https://garden.example/docs/start");
        request.account = "bad role";
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&request, &mut accounting)
                .await
                .unwrap_err(),
            Failure::Admission(Refusal::Invalid)
        );
        assert_eq!(grants.calls.get(), 0);
        assert!(wire.requests.lock().unwrap().is_empty());
    });
}
