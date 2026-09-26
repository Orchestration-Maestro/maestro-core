//! The table of reports refuses, whoever writes, to change, replace or
//! delete a report: a measurement stays as it was recorded.

use super::support::{SYNTHETIC, Scratch, execute, report};
use crate::scope::ScopeSet;

#[test]
fn a_report_is_never_updated_replaced_nor_deleted() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let recorded = database
        .record_eval_report(&report("synthetic", SYNTHETIC))
        .unwrap();
    let id = recorded.id;
    for (statement, refusal) in [
        (
            format!("UPDATE eval_reports SET suite = 'other' WHERE id = '{id}'"),
            "an evaluation report is never updated",
        ),
        (
            format!("DELETE FROM eval_reports WHERE id = '{id}'"),
            "an evaluation report is never deleted",
        ),
        (
            format!(
                "INSERT OR REPLACE INTO eval_reports
                   (id, collection_id, generation_id, suite, digest)
                 SELECT id, collection_id, generation_id, 'other', digest FROM eval_reports
                 WHERE id = '{id}'"
            ),
            "an evaluation report is never replaced",
        ),
    ] {
        let error = execute(&database, &statement).unwrap_err();
        assert!(error.to_string().contains(refusal), "{statement}: {error}");
    }
    let scopes = ScopeSet::default_workspace();
    assert_eq!(
        database.eval_report(&scopes, id).unwrap(),
        Some(recorded),
        "the report is as it was recorded"
    );
}

#[test]
fn a_report_is_never_replaced_through_its_rowid() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let recorded = database
        .record_eval_report(&report("synthetic", SYNTHETIC))
        .unwrap();
    let id = recorded.id;
    // A new id passes the insert's trigger; the rowid is the report's own.
    let statement = format!(
        "INSERT OR REPLACE INTO eval_reports
           (rowid, id, collection_id, generation_id, suite, digest)
         SELECT rowid, '01ARZ3NDEKTSV4RRFFQ69G5FAV', collection_id, generation_id, 'other', digest
         FROM eval_reports WHERE id = '{id}'"
    );
    assert_eq!(
        execute(&database, &statement).map_err(|error| error.to_string()),
        Err("an evaluation report is never deleted: it is a measurement".to_owned())
    );
    let scopes = ScopeSet::default_workspace();
    assert_eq!(database.eval_report(&scopes, id).unwrap(), Some(recorded));
}
