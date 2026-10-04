//! Mutation-debt contracts for strict scalar decoding and calendar boundaries.
use super::{shape, utc};
use crate::Refusal;
use schemars::schema_for;
use serde::Deserialize;
use serde_json::Value;
use std::time::UNIX_EPOCH;

/// Exercise the actual path field decoder, not its validator alone.
#[derive(Debug, Deserialize)]
struct PathField(#[serde(deserialize_with = "shape::path")] String);
/// Exercise the actual DNS-host field decoder.
#[derive(Debug, Deserialize)]
struct HostField(#[serde(deserialize_with = "shape::host")] String);

#[test]
fn policy_path_decode_preserves_and_canonicalizes() {
    for (input, expected) in [("/docs", "/docs"), ("/docs/%c3%a9", "/docs/%C3%A9")] {
        let decoded = serde_json::from_str::<PathField>(&serde_json::to_string(input).unwrap());
        assert!(
            decoded.is_ok(),
            "valid path must decode and consume its input"
        );
        assert_eq!(decoded.unwrap().0, expected);
    }
    for input in ["/.", "/..", "/docs/.", "/docs/.."] {
        assert!(!shape::valid_path(input), "{input}");
    }
}

#[test]
fn policy_host_hyphen_and_url_independent_refusals() {
    let host = serde_json::from_str::<HostField>("\"a-b.example\"");
    assert!(host.is_ok());
    let host = host.unwrap();
    assert_eq!(host.0, "a-b.example");
    for input in [
        "https://garden.example/docs\\bad",
        "https://garden.example/docs?q=back\\slash",
        "https://garden.example/docs bad",
        "https://garden.example/docs\tbad",
        "https://user@garden.example/docs",
        "https://user:password@garden.example/docs",
        "http://garden.example/docs",
        "https://127.0.0.1/docs",
    ] {
        assert!(shape::checked_url(input).is_none(), "{input}");
    }
    let at_limit = format!(
        "https://garden.example/{}",
        "a".repeat(8192 - "https://garden.example/".len())
    );
    assert_eq!(at_limit.len(), 8192);
    assert!(shape::checked_url(&at_limit).is_none()); // The independent path ceiling is 4096.
    let query = format!(
        "https://garden.example/?q={}",
        "a".repeat(8192 - "https://garden.example/?q=".len())
    );
    assert_eq!(query.len(), 8192);
    assert!(shape::checked_url(&query).is_some());
    assert!(shape::checked_url(&(query + "a")).is_none());
}

#[test]
fn policy_calendar_and_clock_edges() {
    for text in [
        "2000-02-29T23:59:59Z",
        "2004-02-29T00:00:00Z",
        "2100-02-28T00:00:00Z",
        "2026-01-01T00:00:00.1Z",
    ] {
        assert!(shape::valid_time(text), "{text}");
    }
    for text in [
        "2100-02-29T00:00:00Z",
        "2001-02-29T00:00:00Z",
        "2026-01-00T00:00:00Z",
        "2026-01-01T24:00:00Z",
        "2026-01-01T00:60:00Z",
        "2026-01-01T00:00:60Z",
        "2026-01-01T00:00:00.Z",
        "2026-01-01T00:00:00.xZ",
        "2026-01-01T0:00:00Z",
    ] {
        assert!(!shape::valid_time(text), "{text}");
    }
}

#[test]
fn policy_retry_after_independent_wire_fields() {
    assert_eq!(
        utc::retry_after("Thu, 01 Jan 1970 00:00:01 GMT", UNIX_EPOCH),
        Ok(1000)
    );
    for text in [
        "Bad, 01 Jan 1970 00:00:01 GMT",
        "Thu, 01 Jan 1970 00:00:01 UTC",
        "Thu, 001 Jan 1970 00:00:01 GMT",
        "Thu, 01 Jan 01970 00:00:01 GMT",
    ] {
        assert_eq!(
            utc::retry_after(text, UNIX_EPOCH),
            Err(Refusal::Invalid),
            "{text}"
        );
    }
}

#[test]
fn policy_required_nullable_schema_name_is_type_specific() {
    let string = schema_for!(shape::RequiredNullable<String>);
    let number = schema_for!(shape::RequiredNullable<u64>);
    assert_eq!(
        string.get("title").and_then(Value::as_str),
        Some("RequiredNullable_string")
    );
    assert_eq!(
        number.get("title").and_then(Value::as_str),
        Some("RequiredNullable_uint64")
    );
}
