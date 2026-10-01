//! What grants reach: the scopes a set was granted, and those it covers of
//! the scopes the kernel knows, its workspace and each recorded collection
//! and source, which `maestro doctor` compares each grant with.

use super::support::{Scratch, scope};
use crate::{
    document::{Collection, Source},
    scope::{Right, Scope, ScopeSet},
    store::Database,
};
use std::collections::BTreeMap;

/// Records the collection `ctm`, with its sources `docs` and `web`, and the
/// collection `other`, with its source `docs`.
fn record(database: &Database) {
    let collections: [(&str, &[&str]); 2] = [("ctm", &["docs", "web"]), ("other", &["docs"])];
    for (collection, sources) in collections {
        database
            .record_collection(&Collection {
                id: collection.to_owned(),
                title: format!("The {collection} collection"),
                visibility: "private".to_owned(),
                profiles: BTreeMap::new(),
            })
            .unwrap();
        for source in sources {
            database
                .record_source(&Source {
                    collection_id: collection.to_owned(),
                    id: (*source).to_owned(),
                    kind: "import".to_owned(),
                    transport: None,
                    reference: format!("corpus_root:{source}.jsonl"),
                    profiles: BTreeMap::new(),
                })
                .unwrap();
        }
    }
}

/// What `principal` reads once granted each of `paths`.
fn granting(database: &Database, principal: &str, paths: &[&str]) -> ScopeSet {
    for path in paths {
        database
            .grant(principal, &scope(path), Right::Read, "test")
            .unwrap();
    }
    database.visible(principal).unwrap()
}

/// The paths of `scopes`, in their order.
fn paths(scopes: &[Scope]) -> Vec<&str> {
    scopes.iter().map(Scope::as_str).collect()
}

#[test]
fn a_set_gives_the_scopes_it_was_granted_in_path_order() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let set = granting(
        &database,
        "reader",
        &["workspace/other", "workspace/default/collection/ctm"],
    );
    let granted: Vec<&str> = set.granted().map(Scope::as_str).collect();
    assert_eq!(
        granted,
        ["workspace/default/collection/ctm", "workspace/other"]
    );
    assert_eq!(database.visible("stranger").unwrap().granted().count(), 0);
}

#[test]
fn the_known_scopes_are_the_workspace_and_each_recorded_collection_and_source() {
    let scratch = Scratch::new();
    let database = scratch.open();
    record(&database);
    let everything = granting(&database, "auditor", &["workspace/default"]);
    assert_eq!(
        paths(&database.known_scopes(&everything).unwrap()),
        [
            "workspace/default",
            "workspace/default/collection/ctm",
            "workspace/default/collection/ctm/source/docs",
            "workspace/default/collection/ctm/source/web",
            "workspace/default/collection/other",
            "workspace/default/collection/other/source/docs",
        ]
    );
}

#[test]
fn a_set_knows_only_the_scopes_it_covers() {
    let scratch = Scratch::new();
    let database = scratch.open();
    record(&database);
    let web = granting(
        &database,
        "web",
        &["workspace/default/collection/ctm/source/web"],
    );
    assert_eq!(
        paths(&database.known_scopes(&web).unwrap()),
        ["workspace/default/collection/ctm/source/web"],
        "a source's grant covers its source, never its collection"
    );
    let ctm = granting(&database, "ctm", &["workspace/default/collection/ctm"]);
    assert_eq!(
        paths(&database.known_scopes(&ctm).unwrap()),
        [
            "workspace/default/collection/ctm",
            "workspace/default/collection/ctm/source/docs",
            "workspace/default/collection/ctm/source/web",
        ]
    );
    let elsewhere = granting(
        &database,
        "elsewhere",
        &[
            "workspace/other",
            "workspace/default/collection/absent",
            "workspace/default/collection/ct",
        ],
    );
    assert_eq!(database.known_scopes(&elsewhere).unwrap(), []);
    assert_eq!(
        database
            .known_scopes(&database.visible("stranger").unwrap())
            .unwrap(),
        []
    );
}

#[test]
fn an_empty_kernel_knows_its_workspace_alone() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let everything = granting(&database, "auditor", &["workspace/default"]);
    assert_eq!(
        paths(&database.known_scopes(&everything).unwrap()),
        ["workspace/default"]
    );
}
