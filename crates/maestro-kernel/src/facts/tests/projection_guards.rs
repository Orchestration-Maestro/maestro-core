//! SQL and scope boundaries for durable projection input pins.
use super::{
    projection::legacy_attached as attached,
    support::{granted, label, on},
};
use crate::{
    facts::{ClaimSet, EXACT_RESOLVER_VERSION, Error, InputMismatchKind, ResolutionInput},
    scope::collection_path,
};
use rusqlite::params;

#[test]
fn raw_projection_receipt_rejects_each_invalid_pin() {
    for (index, column, value) in [
        (2, "settings_identity", None),
        (2, "settings_identity", Some("")),
        (
            2,
            "settings_identity",
            Some("gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg"),
        ),
        (3, "frozen_lock", None),
        (3, "frozen_lock", Some("bad")),
        (
            3,
            "frozen_lock",
            Some("gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg"),
        ),
        (0, "resolution_id", None),
        (0, "resolution_id", Some("bad")),
        (1, "resolver_version", None),
        (1, "resolver_version", Some("unknown/1")),
    ] {
        let (scratch, _database, _scopes, receipt) = attached();
        let connection = scratch.outside();
        let insert = "INSERT INTO graph_projection_receipts
            (generation_id, collection_id, claim_set_id, file_name, schema_version,
             knowledge_edge_count, catalog_dependency_edge_count, entity_fact_count,
             content_digest, resolution_id, resolver_version, settings_identity, frozen_lock)
            VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, 1, ?6, ?7, ?8, ?9, ?10)";
        // Start from a valid INSERT; vary exactly one pin without disabling SQL checks.
        let mut pins = [
            Some(receipt.resolution_id.as_str()),
            Some(receipt.resolver_version.as_str()),
            Some(receipt.settings_identity.as_str()),
            Some(receipt.frozen_lock.as_str()),
        ];
        pins[index] = value;
        assert!(
            connection
                .execute(
                    insert,
                    params![
                        receipt.identity.generation_id,
                        receipt.identity.collection_id,
                        receipt.identity.claim_set_id.as_str(),
                        receipt.identity.file_name,
                        receipt.identity.schema_version,
                        receipt.identity.content_digest.as_str(),
                        pins[0],
                        pins[1],
                        pins[2],
                        pins[3]
                    ]
                )
                .is_err(),
            "{column}={value:?}: raw SQL must reject invalid pins"
        );
        connection
            .execute(
                insert,
                params![
                    receipt.identity.generation_id,
                    receipt.identity.collection_id,
                    receipt.identity.claim_set_id.as_str(),
                    receipt.identity.file_name,
                    receipt.identity.schema_version,
                    receipt.identity.content_digest.as_str(),
                    receipt.resolution_id.as_str(),
                    receipt.resolver_version,
                    receipt.settings_identity.as_str(),
                    receipt.frozen_lock.as_str()
                ],
            )
            .unwrap();
    }
}

#[test]
fn projection_preflight_refuses_hidden_history_set() {
    let (_scratch, database, scopes, receipt) = attached();
    let hidden = database
        .record_claim_set(
            &scopes,
            &ClaimSet {
                collection_id: "other".into(),
                claims: vec![on("rev-o", label())],
            },
        )
        .unwrap();
    let resolution = database
        .record_resolution(
            &scopes,
            "projection",
            &ResolutionInput {
                resolver_version: EXACT_RESOLVER_VERSION.into(),
                sets: vec![receipt.identity.claim_set_id.clone(), hidden.id],
                previous: Some(receipt.resolution_id.clone()),
                decisions: vec![],
            },
            &|_| Ok(()),
        )
        .unwrap();
    let request = granted(&database, "limited", &collection_path("graph"));
    assert!(matches!(
        database
            .validate_projection_inputs(
                &request,
                &receipt.identity.claim_set_id,
                &resolution.id,
                EXACT_RESOLVER_VERSION
            )
            .unwrap_err(),
        Error::ProjectionInputMismatch(InputMismatchKind::Resolution)
    ));
    database
        .validate_projection_inputs(
            &scopes,
            &receipt.identity.claim_set_id,
            &resolution.id,
            EXACT_RESOLVER_VERSION,
        )
        .unwrap();
}
