//! Transfer encoding never changes payload capture identity or provenance label.
use super::{
    n07_parse_url_identity_and_denial_precedence::Controls, n09_decode::compressed, n09_support,
    n12_support::Fixture,
};
use maestro_acquisition::{
    capture::{CaptureEnvelope, Representation, SafeHeader, http_envelope},
    transport::stream::Accounting,
};
use maestro_kernel::acquisition::Receipts;

#[test]
fn n12_gzip_payload_reuses_identity_capture_and_records_transfer_length() {
    let mut fixture = Fixture::new();
    let original = fixture.prepare().unwrap();
    n09_support::run(async {
        let bytes = compressed(true, b"body");
        let policy = n09_support::policy();
        let controls = Controls::default();
        let grants = n09_support::Grants::default();
        let dns = n09_support::Dns::default();
        let wire = n09_support::Wire::new(vec![n09_support::response(
            200,
            "Content-Encoding: gzip\r\n",
            &bytes,
        )]);
        let mut accounting =
            Accounting::new(policy.policy().sources.first().unwrap().limits.clone());
        let response = n09_support::http(&policy, &controls, &grants, &dns, &wire)
            .fetch(
                &n09_support::fetch("https://garden.example/docs/start"),
                &mut accounting,
            )
            .await
            .unwrap();
        assert_eq!(response.wire_body, bytes);
        http_envelope(&mut fixture.envelope, &response).unwrap();
        assert_eq!(fixture.envelope.length, 4);
        assert_eq!(fixture.envelope.representation, Representation::WireBody);
        assert_eq!(
            fixture.envelope.headers.get("content-encoding"),
            Some(&SafeHeader::Value {
                value: "gzip".into()
            })
        );
        assert_eq!(
            fixture.envelope.headers.get("content-length"),
            Some(&SafeHeader::Value {
                value: bytes.len().to_string()
            })
        );
        assert_eq!(fixture.prepare().unwrap(), original);
        let preserved = fixture.db.read("reader", original).unwrap().unwrap();
        let stored: CaptureEnvelope = serde_json::from_slice(preserved.bytes()).unwrap();
        assert!(stored.headers.is_empty()); // First immutable envelope is never rewritten.
    });
}
