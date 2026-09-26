//! The default set for publication is the latest complete set, ignoring
//! building and failed preparations.

use super::support::{Scratch, new_set};
use crate::scope::ScopeSet;

#[test]
fn latest_complete_chunk_set_ignores_newer_incomplete_sets() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let complete = |id: &str| {
        database.begin_chunk_set(&new_set(id, "ctm")).unwrap();
        let manifest = database.put(b"{}", "application/json").unwrap();
        database.complete_chunk_set(id, &manifest).unwrap();
    };
    complete("complete-1");
    database.begin_chunk_set(&new_set("failed", "ctm")).unwrap();
    database.fail_chunk_set("failed").unwrap();
    database
        .begin_chunk_set(&new_set("building", "ctm"))
        .unwrap();
    complete("complete-2");

    assert_eq!(
        database
            .latest_complete_chunk_set(&ScopeSet::default_workspace(), "ctm")
            .unwrap()
            .unwrap()
            .id,
        "complete-2"
    );
}
