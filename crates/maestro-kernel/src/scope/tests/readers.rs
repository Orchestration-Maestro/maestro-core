//! Readers of scoped data take the caller's `ScopeSet` and filter inside
//! their query: nothing outside the set is read, and an empty set reads
//! nothing.

use super::support::{CTM, Scratch, read, record_in, scope};
use crate::{
    journal::{Event, Filter},
    scope::Right,
    store::Database,
};

/// The stream the tests record every event on, whatever its scope.
const STREAM: &str = "import/1";

/// Records one event on [`STREAM`] in each scope of `paths`.
fn record_each(database: &Database, paths: &[&str]) -> Vec<Event> {
    paths
        .iter()
        .map(|path| record_in(database, STREAM, path))
        .collect()
}

#[test]
fn events_are_read_only_inside_the_set() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let recorded = record_each(
        &database,
        &[
            "workspace/default",
            CTM,
            "workspace/default/collection/ctm/source/docs-core",
            "workspace/default/collection/ctm-archive",
            "workspace/default/collection/garden",
            "workspace/other/collection/ctm",
        ],
    );
    database
        .grant("local", &scope(CTM), Right::Read, "test")
        .unwrap();
    let local = database.visible("local").unwrap();
    assert_eq!(read(&database, &local, STREAM), recorded[1..3]);
    let garden = scope("workspace/default/collection/garden");
    database
        .grant("local", &garden, Right::Read, "test")
        .unwrap();
    let wider = database.visible("local").unwrap();
    let seen = [&recorded[1], &recorded[2], &recorded[4]].map(Clone::clone);
    assert_eq!(read(&database, &wider, STREAM), seen);
    let typed = database
        .events(
            &wider,
            &Filter {
                stream: STREAM,
                after: 2,
                r#type: Some("maestro.knowledge.import.completed.v1"),
            },
        )
        .unwrap();
    assert_eq!(typed, seen[1..]);
}

#[test]
fn a_name_that_is_a_prefix_of_another_reads_none_of_it() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let recorded = record_each(
        &database,
        &[
            "workspace/default/collection/ct",
            "workspace/default/collection/ct/source/docs",
            CTM,
            "workspace/default/collection/ctm/source/docs",
            "workspace/default/collection/ct-archive",
        ],
    );
    let ct = scope("workspace/default/collection/ct");
    database.grant("local", &ct, Right::Read, "test").unwrap();
    let local = database.visible("local").unwrap();
    assert_eq!(read(&database, &local, STREAM), recorded[..2]);
}

#[test]
fn an_empty_set_reads_nothing() {
    let scratch = Scratch::new();
    let database = scratch.open();
    record_each(
        &database,
        &[
            "workspace/default",
            CTM,
            "workspace/default/collection/ctm/source/docs-core",
            "",
            "not a scope",
        ],
    );
    let nothing = database.visible("nobody").unwrap();
    assert!(nothing.is_empty());
    assert_eq!(read(&database, &nothing, STREAM), []);
}
