//! Secret canaries across envelopes, hops, journal output, errors and files.
use super::{
    n07_parse_url_identity_and_denial_precedence::Controls, n09_support, n12_support::Fixture,
};
use maestro_acquisition::{
    capture::{Captures, SafeIdentity, http_envelope},
    transport::stream::Accounting,
};
use maestro_kernel::{
    acquisition::{Receipts, safe_headers},
    journal::Filter,
};
use reqwest::header::{HeaderMap, HeaderValue};
use std::{fs, path::Path};

#[test]
fn n12_signed_redirect_chain_has_no_query_canaries_in_any_sink() {
    let mut fixture = Fixture::new();
    n09_support::run(async {
        let policy = n09_support::policy_with(|wire| {
            wire["sources"][0]["identity"]["meaningful_queries"] = serde_json::json!(["token"]);
        });
        let controls = Controls::default();
        let grants = n09_support::Grants::default();
        let dns = n09_support::Dns::default();
        let wire = n09_support::Wire::new(vec![
            n09_support::response(302, "Location: /docs/hop?token=HOP_CANARY\r\n", b"hop"),
            n09_support::response(302, "Location: /docs/final?token=FINAL_CANARY\r\n", b"hop"),
            n09_support::response(
                200,
                "Content-Type: text/html; token=TYPE_CANARY\r\n\
                 ETag: ETAG_CANARY\r\nSet-Cookie: COOKIE_CANARY\r\n",
                b"body",
            ),
        ]);
        let mut accounting =
            Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
        let response = n09_support::http(&policy, &controls, &grants, &dns, &wire)
            .fetch(
                &n09_support::fetch("https://garden.example/docs/start"),
                &mut accounting,
            )
            .await
            .unwrap();
        assert_eq!(response.redirects.len(), 2);
        assert_eq!(response.headers.len(), 3);
        http_envelope(&mut fixture.envelope, &response).unwrap();
        assert!(!format!("{response:?}").contains("CANARY"));
    });
    let capture = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, capture)
        .unwrap();
    let stored = fixture.db.read("reader", capture).unwrap().unwrap();
    assert!(!String::from_utf8_lossy(stored.bytes()).contains("CANARY"));
    let stream = format!("job/{}", fixture.context.writer.token);
    let events = fixture
        .db
        .events(
            &fixture.db.visible("reader").unwrap(),
            &Filter {
                stream: &stream,
                after: 0,
                r#type: None,
            },
        )
        .unwrap();
    assert!(!format!("{events:?}").contains("CANARY"));
    assert_eq!(
        events
            .iter()
            .filter(|event| event.r#type == "maestro.acquisition.acknowledged.v1")
            .count(),
        1
    );
    assert_files_clean(&fixture.root);
}
#[test]
fn n12_identity_deserialization_refuses_unredacted_and_logs_no_secrets() {
    for url in [
        "https://user:USER_CANARY@example.test/docs",
        "https://example.test/docs?token=QUERY_CANARY",
        "https://example.test/docs#FRAGMENT_CANARY",
        "http://example.test/docs",
        "file:///tmp/URL_CANARY",
    ] {
        let wire = format!("{{\"url\":\"{url}\",\"query_digest\":null}}");
        let parsed = serde_json::from_str::<SafeIdentity>(&wire);
        assert!(parsed.is_err(), "unredacted or non-HTTPS identity refuses");
        let error = parsed.unwrap_err();
        assert!(!format!("{error:?} {error}").contains("CANARY"));
    }
    assert!(SafeIdentity::new("invalid URL_CANARY").is_err());
}
#[test]
fn n12_duplicate_or_binary_headers_are_hashed_not_displayed() {
    let mut headers = HeaderMap::new();
    headers.append("content-type", HeaderValue::from_static("text/html"));
    headers.append("content-type", HeaderValue::from_static("TYPE_CANARY"));
    headers.insert("etag", HeaderValue::from_bytes(b"\xffETAG_CANARY").unwrap());
    headers.insert("content-length", HeaderValue::from_static("LENGTH_CANARY"));
    headers.insert("content-encoding", HeaderValue::from_static("CODEC_CANARY"));
    let selected = safe_headers(&headers).unwrap();
    assert_eq!(selected.len(), 4);
    assert!(!format!("{selected:?}").contains("CANARY"));
    assert!(
        !String::from_utf8(serde_json::to_vec(&selected).unwrap())
            .unwrap()
            .contains("CANARY")
    );
}
/// Scan actual artifacts, SQLite and WAL bytes, not a mock serialization sink.
fn assert_files_clean(root: &Path) {
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            assert_files_clean(&path);
        } else {
            assert!(!String::from_utf8_lossy(&fs::read(path).unwrap()).contains("CANARY"));
        }
    }
}
