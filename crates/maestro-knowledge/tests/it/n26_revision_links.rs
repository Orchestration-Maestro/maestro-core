//! N26 kernel relation guards and parity with the existing S1 write.
#![cfg(test)]
use crate::n26_support::{DOCUMENT_ID, Fixture};
use maestro_kernel::{
    acquisition::{Handle, MappedRevision, Receipts, RevisionLink},
    document::{Disposition, Outcome},
    scope::{Right, Scope},
};
use maestro_knowledge::import::{AssetRecord, Imported, ingest_mapped};
#[test]
fn n26_link_visibility_immutability_and_failed_insert_rollback() {
    let fixture = Fixture::new();
    let mut input = fixture.input();
    input.evidence.fidelity = Handle::new();
    input.disposition = Some(Disposition {
        revision_id: String::new(),
        outcome: Outcome::Quarantined,
        reasons: vec!["synthetic hold".into()],
        rule_ids: vec!["test.hold".into()],
        decided_by: "test".into(),
    });
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "missing evidence refuses link insert"
    );
    assert!(
        fixture.revisions().is_empty(),
        "link and revision share a transaction"
    );
    assert!(
        fixture
            .db
            .document(&fixture.scopes, DOCUMENT_ID)
            .unwrap()
            .is_none(),
        "failed native ingestion must not reserve document identity"
    );
    let sql = fixture.sql();
    assert_eq!(
        sql.query_row("SELECT count(*) FROM quality_dispositions", [], |row| row
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    assert_eq!(
        sql.query_row(
            "SELECT count(*) FROM acquisition_revision_links",
            [],
            |row| row.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    let other: Scope = "workspace/default/collection/other/source/docs"
        .parse()
        .unwrap();
    let mut input = fixture.input();
    input.evidence.fidelity = fixture.db.retain(&other, b"wrong scope", &[]).unwrap();
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "link scope mismatch"
    );
    assert!(fixture.revisions().is_empty());
    assert_eq!(
        ingest_mapped(&fixture.target(), fixture.input()).unwrap(),
        Imported::New
    );
    let revision = fixture.revisions().remove(0);
    fixture
        .db
        .grant("outsider", &other, Right::Read, "test")
        .unwrap();
    let denied = fixture.db.visible("outsider").unwrap();
    assert!(
        fixture
            .db
            .revision_links(&denied, &revision.id)
            .unwrap()
            .is_empty()
    );
    assert!(
        fixture
            .db
            .capture_revisions(&denied, fixture.evidence.capture)
            .unwrap()
            .is_empty()
    );
    assert!(
        sql.execute(
            "UPDATE acquisition_revision_links SET fidelity = capture",
            []
        )
        .is_err()
    );
    assert!(
        sql.execute("DELETE FROM acquisition_revision_links", [])
            .is_err()
    );
    assert!(
        sql.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=OFF;
             INSERT OR REPLACE INTO acquisition_revision_links
                 (revision, scope, capture, fidelity, inventory)
             SELECT revision, scope, capture, capture, inventory
             FROM acquisition_revision_links;"
        )
        .is_err(),
        "a replacement must not overwrite immutable fidelity"
    );
}

#[test]
fn n26_inventory_asset_pins_survive_collection_without_replay_leaks() {
    let fixture = Fixture::new();
    let mut input = fixture.input();
    let asset = fixture.available(b"retained only by inventory");
    let digest = asset.digest.clone().unwrap();
    input.assets = vec![
        asset.clone(),
        AssetRecord {
            destination: "required.bin".into(),
            ..asset
        },
    ];
    assert_eq!(fixture.db.artifact(&digest).unwrap().unwrap().pins, 0);
    assert_eq!(
        ingest_mapped(&fixture.target(), input.clone()).unwrap(),
        Imported::New
    );
    assert_eq!(
        fixture.db.artifact(&digest).unwrap().unwrap().pins,
        1,
        "each distinct digest gets one per-link pin"
    );
    assert_eq!(
        ingest_mapped(&fixture.target(), input).unwrap(),
        Imported::Unchanged
    );
    assert_eq!(
        fixture.db.artifact(&digest).unwrap().unwrap().pins,
        1,
        "replay must not leak pins"
    );
    fixture.db.collect_garbage().unwrap();
    assert_eq!(
        fixture.db.get(&digest).unwrap(),
        b"retained only by inventory"
    );
    let revision = fixture.revisions().remove(0);
    let link = fixture
        .db
        .revision_links(&fixture.scopes, &revision.id)
        .unwrap()
        .remove(0);
    assert!(fixture.db.get(&link.inventory).is_ok());
}

#[test]
fn n26_native_and_corpus_revision_disposition_transactions_have_parity() {
    let native = Fixture::new();
    let corpus = Fixture::new();
    let mut input = native.input();
    input.disposition = Some(Disposition {
        revision_id: "caller cannot choose the revision".into(),
        outcome: Outcome::Quarantined,
        reasons: vec!["synthetic hold".into()],
        rule_ids: vec!["test.hold".into()],
        decided_by: "test".into(),
    });
    assert_eq!(
        ingest_mapped(&native.target(), input).unwrap(),
        Imported::Held
    );
    let revision = native.revisions().remove(0);
    let disposition = native
        .db
        .disposition(&native.scopes, &revision.id)
        .unwrap()
        .unwrap();
    for (digest, media) in [
        (&revision.original_digest, "text/markdown"),
        (&revision.canonical_digest, "application/json"),
    ] {
        assert_eq!(
            &corpus
                .db
                .put(&native.db.get(digest).unwrap(), media)
                .unwrap(),
            digest
        );
    }
    corpus
        .db
        .record_document(
            &native
                .db
                .document(&native.scopes, &revision.document_id)
                .unwrap()
                .unwrap(),
        )
        .unwrap();
    corpus
        .db
        .record_revision_with_disposition(&revision, &disposition)
        .unwrap();
    assert_eq!(corpus.revisions(), native.revisions());
    assert_eq!(
        corpus.db.disposition(&corpus.scopes, &revision.id).unwrap(),
        Some(disposition)
    );
    assert_eq!(
        ingest_mapped(&native.target(), native.input()).unwrap(),
        Imported::Unchanged
    );
}

#[test]
fn n26_link_replay_conflicts_and_unknown_inventory_domain_refuse() {
    let fixture = Fixture::new();
    assert_eq!(
        ingest_mapped(&fixture.target(), fixture.input()).unwrap(),
        Imported::New
    );
    let revision = fixture.revisions().remove(0);
    let scope: Scope = "workspace/default/collection/manuals/source/docs"
        .parse()
        .unwrap();
    let mut input = fixture.input();
    input.evidence.fidelity = fixture
        .db
        .retain(&scope, b"substituted fidelity", &[])
        .unwrap();
    assert!(
        ingest_mapped(&fixture.target(), input).is_err(),
        "same revision/capture cannot replace fidelity"
    );
    let mut link = fixture
        .db
        .revision_links(&fixture.scopes, &revision.id)
        .unwrap()
        .remove(0);
    let mut other = revision.clone();
    other.id = "unknown-inventory-revision".into();
    link.revision = other.id.clone();
    link.inventory = fixture
        .db
        .put(br#"["maestro.native_assets/2",[]]"#, "application/json")
        .unwrap();
    assert!(
        fixture
            .db
            .record_mapped_revision(
                MappedRevision {
                    revision: &other,
                    disposition: None,
                    document: None,
                    link: &link
                },
                &scope,
            )
            .is_err(),
        "unknown inventory domain refuses before write"
    );
    assert!(
        fixture
            .db
            .revision(&fixture.scopes, &other.id)
            .unwrap()
            .is_none()
    );
    link.inventory = fixture
        .db
        .put(
            br#"["maestro.native_assets/1",[["x","invented",null,null]]]"#,
            "application/json",
        )
        .unwrap();
    assert!(
        fixture
            .db
            .record_mapped_revision(
                MappedRevision {
                    revision: &other,
                    disposition: None,
                    document: None,
                    link: &link
                },
                &scope,
            )
            .is_err(),
        "unknown asset status refuses"
    );
    link.inventory = fixture
        .db
        .put(
            br#"["maestro.native_assets/1",[["x","missing",null,null,"extra"]]]"#,
            "application/json",
        )
        .unwrap();
    assert!(
        fixture
            .db
            .record_mapped_revision(
                MappedRevision {
                    revision: &other,
                    disposition: None,
                    document: None,
                    link: &link
                },
                &scope,
            )
            .is_err(),
        "non-tuple payload refuses"
    );
}

#[test]
fn n26_link_revision_and_document_source_scope_must_match() {
    let fixture = Fixture::new();
    assert_eq!(
        ingest_mapped(&fixture.target(), fixture.input()).unwrap(),
        Imported::New
    );
    let revision = fixture.revisions().remove(0);
    let scope: Scope = "workspace/default/collection/manuals/source/docs"
        .parse()
        .unwrap();
    let link = fixture
        .db
        .revision_links(&fixture.scopes, &revision.id)
        .unwrap()
        .remove(0);
    let mut other = revision.clone();
    other.id = "mismatched-link-revision".into();
    assert!(
        fixture
            .db
            .record_mapped_revision(
                MappedRevision {
                    revision: &other,
                    disposition: None,
                    document: None,
                    link: &link
                },
                &scope,
            )
            .is_err(),
        "link must name the exact revision"
    );
    assert!(
        fixture
            .db
            .revision(&fixture.scopes, &other.id)
            .unwrap()
            .is_none()
    );
    let mut source = fixture
        .db
        .source(&fixture.scopes, "manuals", "docs")
        .unwrap()
        .unwrap();
    source.id = "other".into();
    fixture.db.record_source(&source).unwrap();
    let mut document = fixture
        .db
        .document(&fixture.scopes, &revision.document_id)
        .unwrap()
        .unwrap();
    document.id = "other-source-document".into();
    document.source_id = source.id;
    document.source_ref = "other-source".into();
    fixture.db.record_document(&document).unwrap();
    other.document_id = document.id;
    let link = RevisionLink {
        revision: other.id.clone(),
        ..link
    };
    assert!(
        fixture
            .db
            .record_mapped_revision(
                MappedRevision {
                    revision: &other,
                    disposition: None,
                    document: None,
                    link: &link
                },
                &scope,
            )
            .is_err(),
        "capture scope must equal revision document scope"
    );
    assert!(
        fixture
            .db
            .revision(&fixture.scopes, &other.id)
            .unwrap()
            .is_none()
    );
}

#[test]
fn n26_link_reads_require_current_transitive_evidence_grants() {
    let fixture = Fixture::new();
    let source: Scope = "workspace/default/collection/manuals/source/docs"
        .parse()
        .unwrap();
    let other: Scope = "workspace/default/collection/other/source/docs"
        .parse()
        .unwrap();
    let child = fixture
        .db
        .retain(&other, b"protected dependency", &[])
        .unwrap();
    let mut input = fixture.input();
    input.evidence.fidelity = fixture
        .db
        .retain(&source, b"cross-scope fidelity", &[child])
        .unwrap();
    assert_eq!(
        ingest_mapped(&fixture.target(), input).unwrap(),
        Imported::New
    );
    let revision = fixture.revisions().remove(0);
    fixture
        .db
        .grant("limited", &source, Right::Read, "test")
        .unwrap();
    let scopes = fixture.db.visible("limited").unwrap();
    assert!(
        fixture
            .db
            .revision_links(&scopes, &revision.id)
            .unwrap()
            .is_empty(),
        "both evidence closures need current grants"
    );
    assert!(
        fixture
            .db
            .capture_revisions(&scopes, fixture.evidence.capture)
            .unwrap()
            .is_empty()
    );
    fixture
        .db
        .grant("limited", &other, Right::Read, "test")
        .unwrap();
    let scopes = fixture.db.visible("limited").unwrap();
    assert_eq!(
        fixture
            .db
            .revision_links(&scopes, &revision.id)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn n26_equal_recapture_adds_a_link_without_overwriting_old_evidence() {
    let fixture = Fixture::new();
    assert_eq!(
        ingest_mapped(&fixture.target(), fixture.input()).unwrap(),
        Imported::New
    );
    let revision = fixture.revisions().remove(0);
    let old = fixture
        .db
        .revision_links(&fixture.scopes, &revision.id)
        .unwrap()
        .remove(0);
    let mut input = fixture.input();
    input.evidence = fixture.recapture();
    let capture = input.evidence.capture;
    assert_eq!(
        ingest_mapped(&fixture.target(), input).unwrap(),
        Imported::Unchanged
    );
    assert_eq!(fixture.revisions(), vec![revision.clone()]);
    let links = fixture
        .db
        .revision_links(&fixture.scopes, &revision.id)
        .unwrap();
    assert_eq!(links.len(), 2);
    assert!(links.contains(&old));
    let new = fixture
        .db
        .capture_revisions(&fixture.scopes, capture)
        .unwrap();
    assert_eq!(new.len(), 1);
    assert_eq!(new[0].inventory, old.inventory);
}
