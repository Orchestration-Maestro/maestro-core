//! N26 failed mapped writes leave no document, revision or transaction effects.
use crate::n26_support::{DOCUMENT_ID, Fixture};
use maestro_kernel::document::{Disposition, Outcome};
use maestro_knowledge::import::ingest_mapped;

#[test]
fn n26_link_insert_failure_rolls_back_revision_disposition_journal_and_pins() {
    let fixture = Fixture::new();
    let mut input = fixture.input();
    input.assets = vec![fixture.available(b"inventory-only bytes")];
    let digest = input.assets[0].digest.clone().unwrap();
    input.disposition = Some(Disposition {
        revision_id: String::new(),
        outcome: Outcome::Quarantined,
        reasons: vec!["synthetic hold".into()],
        rule_ids: vec!["test.hold".into()],
        decided_by: "test".into(),
    });
    let sql = fixture.sql();
    let events: i64 = sql
        .query_row("SELECT count(*) FROM events", [], |row| row.get(0))
        .unwrap();
    sql.execute_batch(
        "CREATE TRIGGER test_link_refusal BEFORE INSERT ON acquisition_revision_links
         BEGIN SELECT RAISE(ABORT, 'synthetic insert failure'); END;",
    )
    .unwrap();
    assert!(ingest_mapped(&fixture.target(), input).is_err());
    assert!(
        fixture.revisions().is_empty(),
        "failed link insert must roll back the revision"
    );
    assert!(
        fixture
            .db
            .document(&fixture.scopes, DOCUMENT_ID)
            .unwrap()
            .is_none(),
        "failed link insert must roll back the document"
    );
    assert_eq!(
        sql.query_row("SELECT count(*) FROM quality_dispositions", [], |row| row
            .get::<_, i64>(
            0
        ))
        .unwrap(),
        0
    );
    assert_eq!(
        sql.query_row("SELECT count(*) FROM events", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        events
    );
    assert_eq!(fixture.db.artifact(&digest).unwrap().unwrap().pins, 0);
}
