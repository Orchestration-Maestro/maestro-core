//! Every consumer receives the kernel's own scoped capture and parent guarantees.
#![cfg(test)]
use super::n37_capture_support::Fixture;
use maestro_kernel::acquisition::{
    Captures, Frontier, ReceiptError, Representation, SafeIdentity, Transport,
};
use std::{fs, slice};

#[test]
fn n37_verify_capture_requires_acknowledged_exact_item_and_scope() {
    let fixture = Fixture::new();
    let handle = fixture.prepare();
    assert_eq!(
        fixture
            .db
            .verify_capture(&fixture.scope, &fixture.row(), handle),
        Err(ReceiptError::Invalid)
    );
    fixture.acknowledge(handle);
    let row = fixture.row();
    fixture
        .db
        .verify_capture(&fixture.scope, &row, handle)
        .unwrap();
    let foreign = "workspace/default/collection/other".parse().unwrap();
    assert_eq!(
        fixture.db.verify_capture(&foreign, &row, handle),
        Err(ReceiptError::Invalid)
    );
    let mut substituted = row.clone();
    substituted.id = "00000000000000000000000001".parse().unwrap();
    assert_eq!(
        fixture
            .db
            .verify_capture(&fixture.scope, &substituted, handle),
        Err(ReceiptError::Invalid)
    );
}

#[test]
fn n37_capture_page_returns_verified_envelope_and_refresh_removes_current_link() {
    let fixture = Fixture::new();
    let handle = fixture.prepare();
    let row = fixture.row();
    let page = fixture
        .db
        .capture_page(&fixture.scope, slice::from_ref(&row))
        .unwrap();
    let observed = page.get(&row.id).unwrap();
    assert_eq!(observed.handle, handle);
    assert_eq!(observed.envelope, fixture.envelope);
    assert!(!observed.acknowledged);
    fixture.acknowledge(handle);
    let row = fixture.row();
    assert!(
        fixture
            .db
            .capture_page(&fixture.scope, slice::from_ref(&row))
            .unwrap()
            .get(&row.id)
            .unwrap()
            .acknowledged
    );
    fixture
        .db
        .refresh(&fixture.context.writer, row.id, fixture.context.now)
        .unwrap();
    assert!(fixture.row().capture.is_none());
    assert!(
        fixture
            .db
            .capture_page(&fixture.scope, slice::from_ref(&row))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn n37_oversized_retained_body_is_invalid_not_generic_storage_failure() {
    let fixture = Fixture::new();
    let handle = fixture.prepare();
    fs::write(
        fixture.artifact_path(&fixture.envelope.artifact),
        b"grown body",
    )
    .unwrap();
    assert_eq!(
        fixture.db.prepared_for(&fixture.scope, &fixture.row()),
        Err(ReceiptError::Invalid)
    );
    assert_eq!(
        fixture.db.acknowledge_capture(&fixture.context, handle),
        Err(ReceiptError::Invalid)
    );
}

#[test]
fn n37_parent_scope_and_source_are_independently_required() {
    for changed in ["scope", "source"] {
        let mut fixture = Fixture::new();
        let parent = fixture.prepare();
        fixture.acknowledge(parent);
        let parent_item = fixture.context.item.item;
        fixture.derive(parent, Representation::SelectedHtml);
        let child = fixture.prepare();
        fixture.acknowledge(child);
        let sql = fixture.sql();
        if changed == "scope" {
            sql.execute(
                "UPDATE acquisition_evidence SET scope = 'workspace/default/collection/other'
                 WHERE id = ?1",
                [parent.to_string()],
            )
            .unwrap();
        } else {
            fixture
                .db
                .lease_source(
                    "other",
                    &fixture.scope,
                    super::n04_frontier_support::request(fixture.context.now),
                )
                .unwrap();
            sql.execute(
                "UPDATE acquisition_frontier SET source = 'other' WHERE id = ?1",
                [parent_item.to_string()],
            )
            .unwrap();
        }
        assert_eq!(
            fixture
                .db
                .verify_capture(&fixture.scope, &fixture.row(), child),
            Err(ReceiptError::Invalid),
            "changed {changed}"
        );
    }
}

#[test]
fn n37_unacknowledged_parent_cannot_be_consumed() {
    let mut fixture = Fixture::new();
    let parent = fixture.prepare();
    fixture.derive(parent, Representation::SelectedHtml);
    assert_eq!(
        fixture
            .db
            .check_capture(&fixture.context, &fixture.envelope, b"body"),
        Err(ReceiptError::Invalid)
    );
}

#[test]
fn n37_parent_derivation_matrix_admits_only_exact_pairs() {
    for (parent_kind, child_kind, allowed) in [
        (Representation::WireBody, Representation::SelectedHtml, true),
        (Representation::WireBody, Representation::ApiRecord, true),
        (
            Representation::RenderedDom,
            Representation::SelectedHtml,
            true,
        ),
        (
            Representation::RenderedDom,
            Representation::ApiRecord,
            false,
        ),
        (
            Representation::SelectedHtml,
            Representation::SelectedHtml,
            true,
        ),
        (
            Representation::SelectedHtml,
            Representation::ApiRecord,
            false,
        ),
        (Representation::ApiRecord, Representation::ApiRecord, true),
        (
            Representation::ApiRecord,
            Representation::SelectedHtml,
            false,
        ),
    ] {
        let mut fixture = Fixture::new();
        if parent_kind == Representation::RenderedDom {
            fixture.envelope.transport = Transport::BrowserRender;
            fixture.envelope.representation = parent_kind;
        }
        let mut parent = fixture.prepare();
        fixture.acknowledge(parent);
        if parent_kind == Representation::SelectedHtml || parent_kind == Representation::ApiRecord {
            fixture.derive(parent, parent_kind);
            parent = fixture.prepare();
            fixture.acknowledge(parent);
        }
        fixture.derive(parent, child_kind);
        assert_eq!(
            fixture
                .db
                .check_capture(&fixture.context, &fixture.envelope, b"body")
                .is_ok(),
            allowed,
            "{parent_kind:?} -> {child_kind:?}"
        );
    }
}

#[test]
fn n37_parent_cycle_refuses_with_intact_artifact_hashes() {
    let fixture = Fixture::new();
    let parent = fixture.prepare();
    let mut envelope = fixture.envelope.clone();
    envelope.parent = Some(parent);
    envelope.representation = Representation::SelectedHtml;
    let digest = fixture
        .db
        .put(&serde_json::to_vec(&envelope).unwrap(), "application/json")
        .unwrap();
    let sql = fixture.sql();
    sql.execute(
        "UPDATE acquisition_evidence SET artifact = ?2 WHERE id = ?1",
        [parent.to_string(), digest.as_str().to_owned()],
    )
    .unwrap();
    sql.execute(
        "UPDATE acquisition_frontier SET capture = ?2 WHERE id = ?1",
        [
            fixture.context.item.item.to_string(),
            digest.as_str().to_owned(),
        ],
    )
    .unwrap();
    assert_eq!(
        fixture
            .db
            .verify_capture(&fixture.scope, &fixture.row(), parent),
        Err(ReceiptError::Invalid)
    );
}

#[test]
fn n37_unredacted_identity_returns_only_exact_query_free_spelling() {
    let identity = SafeIdentity::new("https://example.test/manual#section").unwrap();
    assert_eq!(identity.unredacted(), Some("https://example.test/manual"));
    let redacted = SafeIdentity::new("https://example.test/manual?q=synthetic").unwrap();
    assert_eq!(redacted.unredacted(), None);
    let persisted = serde_json::to_vec(&redacted).unwrap();
    assert_eq!(
        serde_json::from_slice::<SafeIdentity>(&persisted)
            .unwrap()
            .unredacted(),
        None
    );
}
