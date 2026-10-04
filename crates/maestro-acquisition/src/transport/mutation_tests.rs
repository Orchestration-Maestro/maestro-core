//! Exact transport boundaries used by mutation-debt regressions.
use super::budget::Limits;
use crate::policy::schema::SourcePolicy;
use reqwest::header::{HeaderName, HeaderValue};

/// Synthetic explicit envelope shared by transport unit tests.
pub(super) fn limits() -> Limits {
    let policy: SourcePolicy =
        serde_json::from_slice(include_bytes!("../../tests/fixtures/policy.json")).unwrap();
    let mut limits = policy.aggregate_limits;
    limits.memory_bytes = 10_000_000.try_into().unwrap();
    limits.decode.memory_bytes = limits.memory_bytes;
    limits.requests = 10.try_into().unwrap();
    limits.wire_bytes = 1000.try_into().unwrap();
    limits.decode.expanded_bytes = 10_000.try_into().unwrap();
    limits.decode.expansion_ratio = 10.try_into().unwrap();
    limits
}

#[test]
fn s6t_redirect_reference_spelling() {
    let base = reqwest::Url::parse("https://garden.example/docs/page?old=1").unwrap();
    for (location, expected) in [
        ("https://other.example/a", "https://other.example/a"),
        ("bad://", "bad://"),
        ("https://", "https://"),
        ("mailto:synthetic", "mailto:synthetic"),
        ("#part", "https://garden.example/docs/page?old=1#part"),
        ("", "https://garden.example/docs/page?old=1"),
    ] {
        assert_eq!(
            super::http_protocol::redirect_url(&base, location),
            expected
        );
    }
}

#[test]
fn s6t_parser_workspace_header_fraction() {
    let mut accounting = super::accounting::Accounting::new(limits());
    let (_, headers) = super::http_protocol::parser(&mut accounting).unwrap();
    assert_eq!(headers.bytes, 8192);
    assert_eq!(
        headers.fields,
        8192 / size_of::<(HeaderName, HeaderValue)>()
    );
}

#[test]
fn s6t_pacing_concurrency_is_not_a_default() {
    let mut limits = limits();
    limits.origin_concurrency = 7.try_into().unwrap();
    assert_eq!(
        super::pacing::PacingLimits::compose([&limits])
            .unwrap()
            .concurrency(),
        7
    );
}

#[test]
fn s6t_native_transport_debug_is_redacted_but_nonempty() {
    assert!(
        format!(
            "{:?}",
            super::connect::NativeTlsTransport::from_native_roots().unwrap()
        )
        .contains("NativeTlsTransport")
    );
}
