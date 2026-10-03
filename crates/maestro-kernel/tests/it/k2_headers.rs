//! Exact safe-header policy and redacted identity mutation regressions.
use maestro_kernel::acquisition::{
    HeaderReason, SafeHeader, SafeIdentity, safe_header_names, safe_headers, safe_media,
};
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::json;

#[test]
fn k2_headers_pin_policy_and_canonical_values() {
    assert_eq!(
        safe_header_names().unwrap(),
        [
            "content-type",
            "content-length",
            "etag",
            "last-modified",
            "content-encoding"
        ]
    );
    let mut headers = HeaderMap::new();
    for (name, value) in [
        ("content-type", "text/html"),
        ("content-length", "0004"),
        ("content-encoding", "GZIP, deflate"),
        ("etag", "synthetic"),
        ("authorization", "synthetic"),
    ] {
        headers.insert(name, HeaderValue::from_static(value));
    }
    let safe = safe_headers(&headers).unwrap();
    assert_eq!(safe.len(), 4);
    for (name, value) in [
        ("content-type", "text/html"),
        ("content-length", "4"),
        ("content-encoding", "gzip, deflate"),
    ] {
        assert_eq!(
            safe[name],
            SafeHeader::Value {
                value: value.into()
            }
        );
    }
    assert!(matches!(
        safe["etag"],
        SafeHeader::Hashed {
            reason: HeaderReason::UntrustedValue,
            ..
        }
    ));
    headers.insert(
        "content-type",
        HeaderValue::from_static("text/html; secret=synthetic"),
    );
    assert!(matches!(
        safe_headers(&headers).unwrap()["content-type"],
        SafeHeader::Hashed { .. }
    ));
    assert_eq!(
        safe_media(" Text/HTML; charset=utf-8 "),
        Some("text/html".into())
    );
    assert_eq!(safe_media("application/unknown"), None);
}

#[test]
fn k2_identity_retains_exact_public_spelling_and_heap_charge() {
    let base =
        "https://example.test/manual/long-public-path-with-enough-bytes-to-exceed-a-digest-length";
    for query in ["", "?q=synthetic"] {
        let identity = SafeIdentity::new(&format!("{base}{query}")).unwrap();
        assert_eq!(identity.as_str(), base);
        let wire = serde_json::to_value(&identity).unwrap();
        let url = wire["url"].as_str().unwrap();
        let charge = identity.retained_bytes();
        assert!(charge >= url.len() as u64 + if query.is_empty() { 0 } else { 64 });
        let restored: SafeIdentity = serde_json::from_value(wire).unwrap();
        assert_eq!(
            restored.retained_bytes(),
            restored.as_str().len() as u64 + if query.is_empty() { 0 } else { 64 }
        );
    }
    for url in [
        "https://user:secret@example.test/manual",
        "https://example.test/manual#fragment",
        "HTTPS://EXAMPLE.TEST/manual",
    ] {
        let result =
            serde_json::from_value::<SafeIdentity>(json!({"url": url, "query_digest": null}));
        assert!(result.is_err(), "unredacted identity accepted: {url}");
        let error = result.unwrap_err();
        assert!(error.to_string().contains("unredacted capture identity"));
    }
}
