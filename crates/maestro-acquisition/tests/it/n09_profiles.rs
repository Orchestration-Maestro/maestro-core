//! Checked browser profiles must never silently fall back to content HTTP.
use super::{n07_parse_url_identity_and_denial_precedence as n07, n09_support::*, support};
use maestro_acquisition::{
    CheckedPolicy, Refusal,
    transport::{http::Failure, stream::Accounting},
};
use serde_json::json;

/// One already-qualified synthetic profile, rebound through the actual validator.
#[expect(clippy::indexing_slicing, reason = "authored profile fixture fields")]
fn browser(kind: &str) -> CheckedPolicy {
    let (mut collection, mut catalog) = n07::fixture();
    support::put(
        &mut catalog,
        "policy",
        &serde_json::to_value(policy().policy()).unwrap(),
    );
    let mut profile = support::value(&catalog, "http");
    profile["transport"] = kind.into();
    if kind == "browser_render" {
        profile["readiness"] = json!({
            "document_state": "load",
            "all_of": [{"kind": "element_present", "selector": [{
                "tag": "main", "attributes": []
            }]}],
            "timeout_ms": 5000, "poll_interval_ms": 10, "stable_for_ms": 100
        });
    }
    support::put(&mut catalog, "http", &profile);
    support::rebind(&mut collection, &mut catalog);
    n07::checked(collection, &catalog).unwrap()
}
#[test]
fn n09_browser_content_profile_refuses_http_before_effects() {
    run(async {
        for kind in ["browser_request", "browser_render"] {
            let policy = browser(kind);
            let controls = n07::Controls::default();
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::new(vec![response(200, "", b"content")]);
            let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
            assert_eq!(
                http(&policy, &controls, &grants, &dns, &wire)
                    .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                    .await
                    .unwrap_err(),
                Failure::TransportMismatch
            );
            assert_eq!(grants.calls.get(), 0);
            assert_eq!(dns.calls.get(), 0);
            assert!(wire.requests.lock().unwrap().is_empty());
        }
    });
}
#[test]
fn n09_browser_profile_robots_still_uses_only_http() {
    run(async {
        let policy = browser("browser_render");
        let controls = n07::Controls::default();
        let grants = Grants::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", b"User-agent: *\nAllow: /\n")]);
        let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
        let mut request = fetch("https://garden.example/robots.txt");
        request.robots = true;
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&request, &mut accounting)
                .await
                .unwrap()
                .status,
            200
        );
        let mut content = fetch("https://garden.example/docs/start");
        content.robots = true;
        assert_eq!(
            http(&policy, &controls, &grants, &dns, &wire)
                .fetch(&content, &mut accounting)
                .await
                .unwrap_err(),
            Failure::Admission(Refusal::Access)
        );
        assert_eq!(wire.requests.lock().unwrap().len(), 1);
    });
}
