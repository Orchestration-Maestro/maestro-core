//! Typed golden preimages stay byte-equal under `serde_json/preserve_order`.
use super::n12_support::Fixture;
use maestro_acquisition::capture::{CaptureEnvelope, SafeIdentity};
use maestro_kernel::{
    acquisition::{Handle, safe_headers},
    artifact::Digest,
};
use reqwest::header::{HeaderMap, HeaderValue};

#[test]
fn n12_identity_and_envelope_golden_vectors() {
    let mut fixture = Fixture::new();
    fixture.envelope.source = "docs".into();
    fixture.envelope.profile = Digest::of(b"profile");
    fixture.envelope.item = fixed();
    fixture.envelope.run = fixed();
    fixture.envelope.inputs = fixed();
    fixture.envelope.access = fixed();
    fixture.envelope.decision = fixed();
    assert_eq!(
        serde_json::to_vec(&fixture.envelope).unwrap(),
        include_bytes!("../fixtures/n12-envelope.json")
            .strip_suffix(b"\n")
            .expect("golden fixture ends with exactly one LF")
    );
    assert_eq!(
        fixture.envelope.identity().unwrap().as_str(),
        "890a5b7f85ddd05fed408d34d066538a75e63daaef2b2cf9f02a0f43aea04859"
    );
    let restored: CaptureEnvelope = serde_json::from_slice(
        include_bytes!("../fixtures/n12-envelope.json")
            .strip_suffix(b"\n")
            .expect("golden fixture ends with exactly one LF"),
    )
    .unwrap();
    assert_eq!(restored, fixture.envelope);
    let identity =
        SafeIdentity::new("https://user:password@example.test/docs?token=synthetic#fragment")
            .unwrap();
    assert_eq!(
        serde_json::to_vec(&identity).unwrap(),
        include_bytes!("../fixtures/n12-identity.json")
            .strip_suffix(b"\n")
            .expect("golden fixture ends with exactly one LF")
    );
}
#[test]
fn n12_header_preimage_golden_vector() {
    let mut headers = HeaderMap::new();
    headers.insert("etag", HeaderValue::from_static("synthetic-validator"));
    let safe = safe_headers(&headers).unwrap();
    assert_eq!(
        serde_json::to_vec(&safe).unwrap(),
        include_bytes!("../fixtures/n12-headers.json")
            .strip_suffix(b"\n")
            .expect("golden fixture ends with exactly one LF")
    );
    assert_eq!(
        Digest::of(
            include_bytes!("../fixtures/n12-headers.json")
                .strip_suffix(b"\n")
                .expect("golden fixture ends with exactly one LF")
        )
        .as_str(),
        "be999602e6c33085a27479942b4d0ad30251434b96d8cb11104cd696c80c9e8d"
    );
}
/// Fixed synthetic opaque identity independent of wall time.
fn fixed() -> Handle {
    "00000000000000000000000001".parse().unwrap()
}
