//! What doctor finds but must not touch: the entries of the data directory
//! the kernel does not own, maestro v1's files among them, and the grants
//! of `config.toml` that reach no scope the kernel knows.

use super::{
    super::findings::{foreign_entries, unreached_grants},
    support::Scratch,
};
use maestro_kernel::{
    document::Collection,
    scope::{LOCAL, Right, Scope},
    store::Database,
};
use std::{collections::BTreeMap, fs, path::PathBuf};

#[test]
fn the_entries_the_kernel_does_not_own_are_listed_and_left_untouched() {
    let scratch = Scratch::new();
    let data = scratch.data();
    drop(Database::open_in(&data).unwrap());
    for owned in ["kernel.sqlite3-wal", "kernel.sqlite3-shm"] {
        fs::write(data.join(owned), "").unwrap();
    }
    fs::create_dir(data.join("qdrant")).unwrap();
    fs::create_dir(data.join("artifacts")).unwrap();
    for file in [
        "ledger.sqlite3",
        "ledger.sqlite3-wal",
        "maestro.sock",
        "supervisor.lock",
    ] {
        fs::write(data.join(file), "maestro v1").unwrap();
    }
    fs::create_dir(data.join("material")).unwrap();
    assert_eq!(
        foreign_entries(&data),
        [
            data.join("ledger.sqlite3"),
            data.join("ledger.sqlite3-wal"),
            data.join("maestro.sock"),
            data.join("material"),
            data.join("supervisor.lock"),
        ]
    );
    for file in [
        "ledger.sqlite3",
        "ledger.sqlite3-wal",
        "maestro.sock",
        "supervisor.lock",
    ] {
        assert_eq!(fs::read_to_string(data.join(file)).unwrap(), "maestro v1");
    }
    assert!(data.join("material").is_dir());
}

#[test]
fn a_data_directory_that_is_not_there_holds_nothing_foreign() {
    let scratch = Scratch::new();
    assert_eq!(
        foreign_entries(&scratch.data().join("absent")),
        Vec::<PathBuf>::new()
    );
}

#[test]
fn the_grants_that_reach_no_known_scope_are_listed() {
    let scratch = Scratch::new();
    let database = Database::open_in(&scratch.data()).unwrap();
    database
        .record_collection(&Collection {
            id: "ctm".to_owned(),
            title: "The ctm collection".to_owned(),
            visibility: "private".to_owned(),
            profiles: BTreeMap::new(),
        })
        .unwrap();
    for path in [
        "workspace/default",
        "workspace/default/collection/ctm",
        "workspace/default/collection/absent",
        "workspace/other",
    ] {
        let scope: Scope = path.parse().unwrap();
        database.grant(LOCAL, &scope, Right::Read, "test").unwrap();
    }
    let scopes = database.visible(LOCAL).unwrap();
    let unreached: Vec<String> = unreached_grants(&database, &scopes)
        .unwrap()
        .iter()
        .map(ToString::to_string)
        .collect();
    assert_eq!(
        unreached,
        ["workspace/default/collection/absent", "workspace/other"]
    );
}
