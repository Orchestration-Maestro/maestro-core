//! Listing a collection's generations: every one, whatever its state, in the
//! order they were created, and only inside the caller's scopes.

use super::support::{Scratch, generation_in};
use crate::{
    generation::GenerationState::{Building, Failed, Published, Retired},
    scope::{Right, ScopeSet},
    store::Database,
};

/// What `principal` reads once granted only `scope`.
fn granted(database: &Database, principal: &str, scope: &str) -> ScopeSet {
    let scope = scope.parse().unwrap();
    database
        .grant(principal, &scope, Right::Read, "test")
        .unwrap();
    database.visible(principal).unwrap()
}

#[test]
fn a_collection_lists_every_generation_in_the_order_they_were_created() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let retired = generation_in(&database, "ctm", Retired);
    let other = generation_in(&database, "synthetic", Published);
    let published = generation_in(&database, "ctm", Published);
    let failed = generation_in(&database, "ctm", Failed);
    let building = generation_in(&database, "ctm", Building);
    let everything = ScopeSet::default_workspace();
    let listed: Vec<_> = database
        .generations(&everything, "ctm")
        .unwrap()
        .into_iter()
        .map(|generation| (generation.id, generation.collection_id, generation.state))
        .collect();
    let ctm = || "ctm".to_owned();
    assert_eq!(
        listed,
        [
            (retired, ctm(), Retired),
            (published, ctm(), Published),
            (failed, ctm(), Failed),
            (building, ctm(), Building),
        ]
    );
    let synthetic = database.generations(&everything, "synthetic").unwrap();
    assert_eq!(
        synthetic,
        [database.generation(&everything, other).unwrap().unwrap()]
    );
    assert_eq!(database.generations(&everything, "unknown").unwrap(), []);
}

#[test]
fn generations_are_listed_only_inside_the_scopes_that_cover_their_collection() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let ctm = generation_in(&database, "ctm", Published);
    generation_in(&database, "synthetic", Published);
    let synthetic_reader = granted(
        &database,
        "synthetic-reader",
        "workspace/default/collection/synthetic",
    );
    assert_eq!(database.generations(&synthetic_reader, "ctm").unwrap(), []);
    let ctm_reader = granted(&database, "ctm-reader", "workspace/default/collection/ctm");
    let listed: Vec<i64> = database
        .generations(&ctm_reader, "ctm")
        .unwrap()
        .iter()
        .map(|generation| generation.id)
        .collect();
    assert_eq!(listed, [ctm]);
    let source_reader = granted(
        &database,
        "source-reader",
        "workspace/default/collection/ctm/source/docs",
    );
    assert_eq!(database.generations(&source_reader, "ctm").unwrap(), []);
}
