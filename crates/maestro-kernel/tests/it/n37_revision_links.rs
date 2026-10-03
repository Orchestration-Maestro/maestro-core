//! Historical native lineage requires valid S1 verdicts and current capture evidence.
#![cfg(test)]
use super::n37_capture_support::Fixture;
use maestro_kernel::{
    acquisition::{MappedRevision, Receipts, RevisionLink},
    document::{Collection, Disposition, Document, Outcome, Revision, RevisionStatus, Source},
};
use std::collections::BTreeMap;

#[test]
fn n37_verified_revision_links_return_exact_lineage_and_refuse_failed_revision() {
    for status in [RevisionStatus::Valid, RevisionStatus::Failed] {
        let fixture = Fixture::new();
        let capture = fixture.prepare();
        fixture.acknowledge(capture);
        fixture
            .db
            .record_collection(&Collection {
                id: "docs".into(),
                title: "synthetic".into(),
                visibility: "public".into(),
                profiles: BTreeMap::new(),
            })
            .unwrap();
        fixture
            .db
            .record_source(&Source {
                collection_id: "docs".into(),
                id: "docs".into(),
                kind: "native".into(),
                transport: Some("http".into()),
                reference: "synthetic".into(),
                profiles: BTreeMap::new(),
            })
            .unwrap();
        let document = Document {
            id: "document".into(),
            collection_id: "docs".into(),
            source_id: "docs".into(),
            source_ref: "https://example.test/manual".into(),
        };
        let revision = Revision {
            id: "prior".into(),
            document_id: document.id.clone(),
            original_digest: fixture.db.put(b"original", "text/plain").unwrap(),
            canonical_digest: fixture.db.put(b"canonical", "text/plain").unwrap(),
            status,
            captured_at: None,
            metadata: serde_json::Map::new(),
        };
        let link = RevisionLink {
            revision: revision.id.clone(),
            capture,
            fidelity: fixture
                .db
                .retain(&fixture.scope, b"fidelity", &[capture])
                .unwrap(),
            inventory: fixture
                .db
                .put(br#"["maestro.native_assets/1",[]]"#, "application/json")
                .unwrap(),
        };
        let disposition = Disposition {
            revision_id: revision.id.clone(),
            outcome: Outcome::Accepted,
            reasons: vec![],
            rule_ids: vec![],
            decided_by: "synthetic".into(),
        };
        fixture
            .db
            .record_mapped_revision(
                MappedRevision {
                    revision: &revision,
                    disposition: Some(&disposition),
                    document: Some(&document),
                    link: &link,
                },
                &fixture.scope,
            )
            .unwrap();
        let scopes = fixture.db.visible("reader").unwrap();
        let result = fixture.db.verified_revision_links(&scopes, &revision.id);
        if status == RevisionStatus::Failed {
            assert!(result.is_err(), "failed revision selected");
        } else {
            assert_eq!(result.unwrap(), vec![link]);
        }
        assert!(
            fixture
                .db
                .verified_revision_links(&scopes, "missing")
                .unwrap()
                .is_empty()
        );
    }
}
