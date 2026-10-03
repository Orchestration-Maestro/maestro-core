//! Mutation regressions for immutable capture admission and readback.
use super::{
    n04_frontier_support::{dispatch, item, now},
    n37_capture_support::Fixture,
};
use maestro_kernel::{
    acquisition::{
        Captures, Frontier, Handle, ReceiptError, Receipts, RedirectHop, Representation,
        SafeIdentity, Transport,
    },
    artifact::Digest,
};
use std::slice;

#[test]
fn k2_capture_readback_and_byte_accounting() {
    let fixture = Fixture::new();
    assert_eq!(fixture.db.capture_bytes(&fixture.envelope).unwrap(), 0);
    let encoded = serde_json::to_vec(&fixture.envelope).unwrap();
    let needed = 4 + encoded.len() as u64;
    assert_eq!(
        fixture
            .db
            .check_capture(&fixture.context, &fixture.envelope, b"body")
            .unwrap(),
        needed
    );
    assert_eq!(
        fixture
            .db
            .prepare_capture(&fixture.context, &fixture.envelope, b"body", needed - 1),
        Err(ReceiptError::Conflict)
    );
    let prepared = fixture
        .db
        .prepare_capture(&fixture.context, &fixture.envelope, b"body", needed)
        .unwrap();
    assert_eq!(prepared.retained_bytes, needed);
    assert_eq!(fixture.db.capture_bytes(&fixture.envelope).unwrap(), needed);
    assert_eq!(
        fixture
            .db
            .read_capture(&fixture.context, prepared.handle, 4),
        Ok((fixture.envelope.clone(), Some(b"body".to_vec())))
    );
    assert_eq!(
        fixture
            .db
            .read_capture(&fixture.context, prepared.handle, 3),
        Ok((fixture.envelope.clone(), None))
    );
    assert_eq!(
        fixture
            .db
            .read_capture(&fixture.context, prepared.handle, 0),
        Err(ReceiptError::Invalid)
    );
    assert_eq!(
        fixture
            .db
            .prepare_capture(&fixture.context, &fixture.envelope, b"body", 0)
            .unwrap()
            .handle,
        prepared.handle
    );
    assert!(
        fixture
            .db
            .capture_for(&fixture.scope, &fixture.row())
            .unwrap()
            .is_none()
    );
    fixture.acknowledge(prepared.handle);
    assert_eq!(
        fixture
            .db
            .capture_for(&fixture.scope, &fixture.row())
            .unwrap(),
        Some(prepared.handle)
    );
    let mut row = fixture.row();
    row.capture = None;
    assert!(
        fixture
            .db
            .capture_for(&fixture.scope, &row)
            .unwrap()
            .is_none()
    );
}

#[test]
fn k2_capture_validation_refuses_each_independent_field() {
    let fixture = Fixture::new();
    for field in [
        "digest",
        "length",
        "schema",
        "source",
        "item",
        "status",
        "redirect_count",
        "wire_transport",
        "wire_parent",
        "render_transport",
        "render_parent",
        "derived_parent",
        "declared_media",
        "detected_media",
        "redirect_status",
        "requested",
        "authorization",
        "profile",
    ] {
        let mut envelope = fixture.envelope.clone();
        match field {
            "digest" => envelope.artifact = Digest::of(b"other"),
            "length" => envelope.length = 3,
            "schema" => envelope.schema = "other".into(),
            "source" => envelope.source = "other".into(),
            "item" => envelope.item = Handle::new(),
            "status" => envelope.status = 600,
            "redirect_count" => {
                envelope.redirects = vec![
                    RedirectHop {
                        identity: envelope.requested.clone(),
                        status: 301
                    };
                    1001
                ];
            }
            "wire_transport" => envelope.transport = Transport::BrowserRender,
            "wire_parent" => envelope.parent = Some(Handle::new()),
            "render_transport" => envelope.representation = Representation::RenderedDom,
            "render_parent" => {
                envelope.representation = Representation::RenderedDom;
                envelope.transport = Transport::BrowserRender;
                envelope.parent = Some(Handle::new());
            }
            "derived_parent" => envelope.representation = Representation::SelectedHtml,
            "declared_media" => envelope.declared_media = Some("text/html; token=synthetic".into()),
            "detected_media" => envelope.detected_media = Some("unknown".into()),
            "redirect_status" => envelope.redirects.push(RedirectHop {
                identity: envelope.requested.clone(),
                status: 200,
            }),
            "requested" => {
                envelope.requested = SafeIdentity::new("https://example.test/other").unwrap();
            }
            "authorization" => envelope.authorization_context = Digest::of(b"other"),
            "profile" => envelope.profile = Digest::of(b"other"),
            _ => panic!("unknown fixture field: {field}"),
        }
        assert_eq!(
            fixture
                .db
                .check_capture(&fixture.context, &envelope, b"body"),
            Err(ReceiptError::Invalid),
            "{field}"
        );
    }
    let mut valid = fixture.envelope.clone();
    valid.declared_media = Some("text/html".into());
    valid.detected_media = Some("text/plain".into());
    valid.redirects = vec![
        RedirectHop {
            identity: valid.requested.clone(),
            status: 301
        };
        1000
    ];
    assert!(
        fixture
            .db
            .check_capture(&fixture.context, &valid, b"body")
            .is_ok()
    );
    valid.redirects.clear();
    valid.representation = Representation::RenderedDom;
    valid.transport = Transport::BrowserRender;
    assert!(
        fixture
            .db
            .check_capture(&fixture.context, &valid, b"body")
            .is_ok()
    );
}

#[test]
fn k2_capture_reads_refuse_substituted_context_and_link() {
    let fixture = Fixture::new();
    let handle = fixture.prepare();
    let mut other = item();
    other.fetch_identity = "https://example.test/other".into();
    let row = fixture
        .db
        .enqueue(&fixture.context.writer, &other, now())
        .unwrap();
    let lease = fixture
        .db
        .lease(&fixture.context.writer, row.id, dispatch(now()))
        .unwrap();
    let mut context = fixture.context.clone();
    context.item = lease;
    assert_eq!(
        fixture.db.read_capture(&context, handle, 4),
        Err(ReceiptError::Invalid)
    );
    context = fixture.context.clone();
    context.writer.source = "other".into();
    assert_eq!(
        fixture.db.read_capture(&context, handle, 4),
        Err(ReceiptError::Invalid)
    );
    let substitute = fixture
        .db
        .retain(
            &fixture.scope,
            &serde_json::to_vec(&fixture.envelope).unwrap(),
            &[],
        )
        .unwrap();
    assert_ne!(substitute, handle);
    assert_eq!(
        fixture.db.read_capture(&fixture.context, substitute, 4),
        Err(ReceiptError::Invalid)
    );
    let mut different = fixture.envelope.clone();
    different.artifact = Digest::of(b"next");
    assert_eq!(
        fixture
            .db
            .check_capture(&fixture.context, &different, b"next"),
        Err(ReceiptError::Conflict)
    );
}

#[test]
fn k2_capture_page_bound_and_acknowledged_input_match() {
    let fixture = Fixture::new();
    let handle = fixture.prepare();
    fixture.acknowledge(handle);
    let mut row = fixture.row();
    row.capture = None;
    assert!(
        !fixture
            .db
            .capture_page(&fixture.scope, slice::from_ref(&row))
            .unwrap()[&row.id]
            .acknowledged
    );
    assert!(
        fixture
            .db
            .capture_page(&fixture.scope, &vec![row.clone(); 1000])
            .is_ok()
    );
    assert!(matches!(
        fixture.db.capture_page(&fixture.scope, &vec![row; 1001]),
        Err(ReceiptError::Invalid)
    ));
}

#[test]
fn k2_capture_verifier_rejects_relinked_envelope_item() {
    let fixture = Fixture::new();
    let handle = fixture.prepare();
    let mut envelope = fixture.envelope.clone();
    envelope.item = Handle::new();
    let digest = fixture
        .db
        .put(&serde_json::to_vec(&envelope).unwrap(), "application/json")
        .unwrap();
    fixture
        .sql()
        .execute(
            "UPDATE acquisition_evidence SET artifact = ?2 WHERE id = ?1",
            [handle.to_string(), digest.as_str().into()],
        )
        .unwrap();
    fixture
        .sql()
        .execute(
            "UPDATE acquisition_frontier SET capture = ?2 WHERE id = ?1",
            [
                fixture.context.item.item.to_string(),
                digest.as_str().into(),
            ],
        )
        .unwrap();
    assert_eq!(
        fixture
            .db
            .verify_capture(&fixture.scope, &fixture.row(), handle),
        Err(ReceiptError::Invalid)
    );
}
