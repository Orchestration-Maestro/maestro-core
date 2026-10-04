//! N12's immutable capture, verified acknowledgment and redaction contracts.
use super::n12_support::{Fixture, response};
use maestro_acquisition::capture::{
    CaptureEnvelope, Captures, RedirectHop, Representation, SafeIdentity, http_envelope,
};
use maestro_kernel::{
    acquisition::{Frontier, Handle, ReceiptError, Receipts},
    artifact::Digest,
    store::Database,
};
use std::fs;

#[test]
fn n12_verification_refuses_oversized_raw_before_digest_mismatch() {
    let fixture = Fixture::new();
    let capture = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, capture)
        .unwrap();
    let scopes = fixture.db.visible("reader").unwrap();
    let item = Frontier::page(&fixture.db, &scopes, "notes", None, 10)
        .unwrap()
        .remove(0);
    let scope = "workspace/default/collection/garden".parse().unwrap();
    let hex = fixture.envelope.artifact.as_str();
    let path = fixture
        .root
        .join("artifacts/sha256")
        .join(hex.get(..2).unwrap())
        .join(hex.get(2..4).unwrap())
        .join(hex);
    for (bytes, expected) in [
        (b"xxxx".as_slice(), ReceiptError::Storage),
        (b"oversized".as_slice(), ReceiptError::Invalid),
    ] {
        fs::write(&path, bytes).unwrap();
        assert_eq!(
            fixture.db.verify_capture(&scope, &item, capture),
            Err(expected)
        );
    }
}

#[test]
fn n12_capture_readback_refuses_oversized_raw_before_digest_mismatch() {
    let fixture = Fixture::new();
    fixture.prepare().unwrap();
    let hex = fixture.envelope.artifact.as_str();
    let path = fixture
        .root
        .join("artifacts/sha256")
        .join(hex.get(..2).unwrap())
        .join(hex.get(2..4).unwrap())
        .join(hex);
    for (bytes, expected) in [
        (b"xxxx".as_slice(), ReceiptError::Storage),
        (b"oversized".as_slice(), ReceiptError::Invalid),
    ] {
        fs::write(&path, bytes).unwrap();
        assert_eq!(fixture.db.capture_bytes(&fixture.envelope), Err(expected));
    }
}

#[test]
fn n12_digest_substitution_refuses_before_persistence() {
    let mut fixture = Fixture::new();
    fixture.envelope.artifact = Digest::of(b"substituted");
    assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
    assert!(fixture.db.artifact(&Digest::of(b"body")).unwrap().is_none());
}
#[test]
fn n12_length_substitution_refuses_before_persistence() {
    let mut fixture = Fixture::new();
    fixture.envelope.length = 5;
    assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
    assert!(fixture.db.artifact(&Digest::of(b"body")).unwrap().is_none());
}
#[test]
fn n12_duplicate_reuses_digest_identical_capture() {
    let mut fixture = Fixture::new();
    let first = fixture.prepare().unwrap();
    let recorded = fixture.db.check_artifacts().unwrap().recorded;
    fixture.envelope.observed_ms += 1;
    assert_eq!(fixture.prepare().unwrap(), first);
    assert_eq!(fixture.db.check_artifacts().unwrap().recorded, recorded);
    let body = fixture.db.artifact(&Digest::of(b"body")).unwrap().unwrap();
    assert_eq!(body.pins, 1);
    fixture
        .db
        .acknowledge_capture(&fixture.context, first)
        .unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, first)
        .unwrap();
    assert_eq!(
        fixture
            .db
            .artifact(&Digest::of(b"body"))
            .unwrap()
            .unwrap()
            .pins,
        1
    );
}
#[test]
fn n12_crash_replay_links_once_before_stage_ack() {
    let mut fixture = Fixture::new();
    let first = fixture.prepare().unwrap();
    let scopes = fixture.db.visible("reader").unwrap();
    assert!(
        Frontier::page(&fixture.db, &scopes, "notes", None, 10).unwrap()[0]
            .capture
            .is_none()
    );
    fixture.db = Database::open_in(&fixture.root).unwrap();
    assert_eq!(fixture.prepare().unwrap(), first);
    fixture
        .db
        .acknowledge_capture(&fixture.context, first)
        .unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, first)
        .unwrap();
    let items = Frontier::page(&fixture.db, &scopes, "notes", None, 10).unwrap();
    assert_eq!(items.len(), 1);
    assert!(items[0].capture.is_some());
    assert_eq!(items[0].attempts, 1);
    let bytes = fixture.db.read("reader", first).unwrap().unwrap();
    let envelope: CaptureEnvelope = serde_json::from_slice(bytes.bytes()).unwrap();
    assert_eq!(envelope.artifact, Digest::of(b"body"));
    assert_eq!(fixture.db.get(&envelope.artifact).unwrap(), b"body");
}
#[test]
fn n12_http_cannot_mislabel_dom_or_selection() {
    let response = response();
    for label in [
        Representation::RenderedDom,
        Representation::SelectedHtml,
        Representation::ApiRecord,
    ] {
        let mut fixture = Fixture::new();
        fixture.envelope.representation = label;
        assert_eq!(
            http_envelope(&mut fixture.envelope, &response),
            Err(ReceiptError::Invalid)
        );
        assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
    }
}
#[test]
fn n12_header_allow_list_and_signed_urls_persist_no_secrets() {
    let mut fixture = Fixture::new();
    let response = response();
    http_envelope(&mut fixture.envelope, &response).unwrap();
    fixture.envelope.final_identity = SafeIdentity::new(
        "https://USER_CANARY:PASSWORD_CANARY@garden.example/docs/start?\
        signature=URL_CANARY#FRAGMENT_CANARY",
    )
    .unwrap();
    fixture.envelope.redirects.push(RedirectHop {
        identity: fixture.envelope.final_identity.clone(),
        status: 302,
    });
    assert_eq!(
        fixture
            .envelope
            .headers
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        vec!["content-length", "content-type", "etag", "last-modified"]
    );
    let handle = fixture.prepare().unwrap();
    let bytes = fixture.db.read("reader", handle).unwrap().unwrap();
    let text = String::from_utf8(bytes.bytes().to_vec()).unwrap();
    for secret in [
        "URL_CANARY",
        "FRAGMENT_CANARY",
        "USER_CANARY",
        "PASSWORD_CANARY",
        "HEADER_CANARY",
        "VALIDATOR_CANARY",
        "DATE_CANARY",
        "COOKIE_CANARY",
        "AUTH_CANARY",
        "TRACE_CANARY",
    ] {
        assert!(!text.contains(secret), "persisted {secret}");
        assert!(!format!("{:?}", fixture.envelope).contains(secret));
    }
    assert_eq!(
        fixture.envelope.declared_media.as_deref(),
        Some("text/html")
    );
    assert_eq!(fixture.envelope.length, 4);
}
#[test]
fn n12_stale_lease_and_wrong_handle_cannot_ack() {
    let mut fixture = Fixture::new();
    let handle = fixture.prepare().unwrap();
    assert!(
        fixture
            .db
            .acknowledge_capture(&fixture.context, Handle::new())
            .is_err()
    );
    fixture.context.item.epoch += 1;
    assert!(
        fixture
            .db
            .acknowledge_capture(&fixture.context, handle)
            .is_err()
    );
    assert!(fixture.prepare().is_err());
}
#[test]
fn n12_changed_profile_or_authorization_never_reuses() {
    for auth in [false, true] {
        let mut fixture = Fixture::new();
        if auth {
            fixture.envelope.authorization_context = Digest::of(b"other account");
        } else {
            fixture.envelope.profile = Digest::of(b"other profile");
        }
        assert_eq!(fixture.prepare(), Err(ReceiptError::Invalid));
    }
}
