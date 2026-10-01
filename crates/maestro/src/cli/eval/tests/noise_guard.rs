//! The identifier noise guard on the ladder: a guarded search's row counts
//! the identifiers it dropped.

use super::{
    super::reports::PrivateRow,
    reports::{runs, to_json},
};
use serde_json::Value;

#[test]
fn a_guarded_row_counts_its_dropped_identifiers() {
    let runs = runs();
    let row = &runs[0].rows[0];
    let off = to_json(&PrivateRow::new(
        row,
        &runs[0].diagnostics[0],
        &[],
        None,
        false,
    ));
    assert_eq!(off.get("identifiers_dropped"), None);

    let mut guarded = runs[0].diagnostics[0].clone();
    guarded.identifiers_dropped = 1;
    let on = to_json(&PrivateRow::new(row, &guarded, &[], None, false));
    assert_eq!(on["identifiers_dropped"], Value::from(1));
}
