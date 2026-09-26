//! The collections a set reads, listed whole: those whose scope it covers,
//! in id order, as `maestro status` reports them.

use super::support::{Scratch, collection};
use crate::scope::{Right, ScopeSet};

#[test]
fn the_collections_a_set_covers_are_listed_in_id_order() {
    let scratch = Scratch::new();
    let database = scratch.open();
    for id in ["other", "alpha"] {
        database.record_collection(&collection(id)).unwrap();
    }
    assert_eq!(
        database
            .collections(&ScopeSet::default_workspace())
            .unwrap(),
        [collection("alpha"), collection("ctm"), collection("other")]
    );
    let granted = |principal: &str, path: &str| {
        database
            .grant(principal, &path.parse().unwrap(), Right::Read, "test")
            .unwrap();
        database.visible(principal).unwrap()
    };
    let ctm = granted("ctm-reader", "workspace/default/collection/ctm");
    assert_eq!(database.collections(&ctm).unwrap(), [collection("ctm")]);
    let docs = granted(
        "docs-reader",
        "workspace/default/collection/ctm/source/docs",
    );
    assert_eq!(
        database.collections(&docs).unwrap(),
        [],
        "a source's grant reads its source, never its collection"
    );
    let nothing = database.visible("stranger").unwrap();
    assert_eq!(database.collections(&nothing).unwrap(), []);
}
