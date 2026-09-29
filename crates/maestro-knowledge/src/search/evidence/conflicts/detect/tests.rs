use super::components;
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn conflict_components_are_connected_sets_and_always_consume_the_graph() {
    let graph = BTreeMap::from([
        (1, BTreeSet::from([2])),
        (2, BTreeSet::from([1, 3])),
        (3, BTreeSet::from([2])),
        (7, BTreeSet::new()),
    ]);
    assert_eq!(
        components(&graph),
        [BTreeSet::from([1, 2, 3]), BTreeSet::from([7])]
    );
}
