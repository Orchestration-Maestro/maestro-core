//! Explicit S1 prior revision selection never becomes implicit stage completion.
use super::{n09_support::policy, n12_support::Fixture};
use maestro_acquisition::lifecycle::resume::select_prior;
use maestro_kernel::{
    acquisition::{Captures, Frontier, MappedRevision, Receipts, RevisionLink},
    artifact::Digest,
    document::{Collection, Disposition, Document, Outcome, Revision, RevisionStatus, Source},
    scope::Scope,
};
use rusqlite::Connection;
use std::{collections::BTreeMap, fs};

/// Independently authored S1 revision with the existing N26 capture/fidelity edges.
fn prior() -> (Fixture, String, Scope) {
    prior_with(RevisionStatus::Valid, Outcome::Accepted)
}
/// Current quality and revision verdicts must both permit explicit prior use.
fn prior_with(status: RevisionStatus, outcome: Outcome) -> (Fixture, String, Scope) {
    let scope: Scope = "workspace/default/collection/garden/source/notes"
        .parse()
        .unwrap();
    let fixture = Fixture::with_scope(policy(), &scope);
    let capture = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, capture)
        .unwrap();
    fixture
        .db
        .record_collection(&Collection {
            id: "garden".into(),
            title: "synthetic".into(),
            visibility: "public".into(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    fixture
        .db
        .record_source(&Source {
            collection_id: "garden".into(),
            id: "notes".into(),
            kind: "native".into(),
            transport: Some("http".into()),
            reference: "synthetic".into(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    let document = Document {
        id: "document".into(),
        collection_id: "garden".into(),
        source_id: "notes".into(),
        source_ref: "https://garden.example/docs/start".into(),
    };
    let original = fixture.db.put(b"# prior", "text/markdown").unwrap();
    let canonical = fixture
        .db
        .put(b"canonical prior", "application/json")
        .unwrap();
    let revision = Revision {
        id: "prior-revision".into(),
        document_id: document.id.clone(),
        original_digest: original,
        canonical_digest: canonical,
        status,
        captured_at: None,
        metadata: serde_json::Map::new(),
    };
    let fidelity = fixture
        .db
        .retain(&scope, b"verified fidelity", &[capture])
        .unwrap();
    let inventory = fixture
        .db
        .put(br#"["maestro.native_assets/1",[]]"#, "application/json")
        .unwrap();
    let link = RevisionLink {
        revision: revision.id.clone(),
        capture,
        fidelity,
        inventory,
    };
    let disposition = Disposition {
        revision_id: revision.id.clone(),
        outcome,
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
            &scope,
        )
        .unwrap();
    (fixture, revision.id, scope)
}

#[test]
fn n37_explicit_verified_prior_revision_is_reported_not_completed_or_fallback() {
    use maestro_kernel::acquisition::{DispatchRequest, LeaseRequest};
    use std::time::Duration;
    let (mut fixture, revision, scope) = prior();
    let selected = select_prior(&fixture.db, "reader", &revision).unwrap();
    assert_eq!(selected.revision, revision);
    assert_eq!(selected.links.len(), 1);
    assert_eq!(
        selected.report(),
        "selected prior revision prior-revision, not freshly verified"
    );
    assert!(
        select_prior(&fixture.db, "reader", "missing-revision").is_err(),
        "implicit latest fallback"
    );
    assert!(select_prior(&fixture.db, "denied", &revision).is_err());
    fixture
        .db
        .refresh(
            &fixture.context.writer,
            fixture.context.item.item,
            fixture.context.now,
        )
        .unwrap();
    assert!(
        select_prior(&fixture.db, "reader", &revision).is_err(),
        "historical generation silently accepted as current"
    );
    let row = Frontier::page(
        &fixture.db,
        &fixture.db.visible("reader").unwrap(),
        "notes",
        None,
        1,
    )
    .unwrap()
    .remove(0);
    assert!(row.capture.is_none());
    assert_eq!(fixture.db.capture_for(&scope, &row).unwrap(), None);
    assert_eq!(
        fixture
            .db
            .revision_links(&fixture.db.visible("reader").unwrap(), &revision)
            .unwrap(),
        selected.links,
        "immutable lineage lost"
    );
    fixture.context.item = fixture
        .db
        .lease(
            &fixture.context.writer,
            fixture.context.item.item,
            DispatchRequest {
                lease: LeaseRequest {
                    holder: "worker",
                    now: fixture.context.now,
                    term: Duration::from_secs(30),
                },
                max_attempts: 3,
            },
        )
        .unwrap();
    let equal = fixture.prepare().unwrap();
    fixture
        .db
        .acknowledge_capture(&fixture.context, equal)
        .unwrap();
    assert!(
        select_prior(&fixture.db, "reader", &revision).is_err(),
        "equal bytes bypassed selected prior revision generation check"
    );
}

#[test]
fn n37_prior_revision_rehashes_s1_and_capture_artifacts() {
    for artifact in ["raw", "original", "canonical", "fidelity", "inventory"] {
        let (fixture, revision, _) = prior();
        let scopes = fixture.db.visible("reader").unwrap();
        let record = fixture.db.revision(&scopes, &revision).unwrap().unwrap();
        let link = fixture
            .db
            .revision_links(&scopes, &revision)
            .unwrap()
            .remove(0);
        let digest = match artifact {
            "raw" => fixture.envelope.artifact.clone(),
            "original" => record.original_digest,
            "canonical" => record.canonical_digest,
            "inventory" => link.inventory,
            "fidelity" => {
                let sql = Connection::open(fixture.root.join("kernel.sqlite3")).unwrap();
                let text: String = sql
                    .query_row(
                        "SELECT artifact FROM acquisition_evidence WHERE id = ?1",
                        [link.fidelity.to_string()],
                        |row| row.get(0),
                    )
                    .unwrap();
                Digest::parse(&text).unwrap()
            }
            _ => panic!("unknown synthetic artifact"),
        };
        let hex = digest.as_str();
        let path = fixture
            .root
            .join("artifacts/sha256")
            .join(hex.get(..2).unwrap())
            .join(hex.get(2..4).unwrap())
            .join(hex);
        fs::write(path, b"damaged").unwrap();
        assert!(
            select_prior(&fixture.db, "reader", &revision).is_err(),
            "corrupt prior artifact accepted ({artifact})"
        );
    }
}

#[test]
fn n37_prior_revision_cannot_select_failed_or_held_output() {
    for (status, outcome) in [
        (RevisionStatus::Failed, Outcome::Accepted),
        (RevisionStatus::Valid, Outcome::Quarantined),
        (RevisionStatus::Valid, Outcome::NeedsReextraction),
        (RevisionStatus::Valid, Outcome::Excluded),
    ] {
        let (fixture, revision, _) = prior_with(status, outcome);
        assert!(
            select_prior(&fixture.db, "reader", &revision).is_err(),
            "{status:?}/{outcome:?} selected"
        );
    }
}
