//! A report is stored as an artifact, indexed and pinned by its record, and
//! journaled on its collection's stream, all in one write, for a generation
//! of its own collection only; it is read back only through scopes that
//! cover that collection.

use super::support::{CTM, JSON, SYNTHETIC, Scratch, execute, report};
use crate::{
    artifact::Digest,
    eval::{Error, RECORDED},
    journal::{Event, Filter},
    scope::{Right, ScopeSet},
    store::{self, Database},
};
use serde_json::json;
use std::error;

/// The reports of `collection` that `scopes` reads.
fn ids(database: &Database, scopes: &ScopeSet, collection: &str) -> Vec<String> {
    database
        .eval_reports(scopes, collection)
        .unwrap()
        .into_iter()
        .map(|report| report.id.to_string())
        .collect()
}

/// The events of the kind [`RECORDED`] on the stream of `collection`.
fn recorded_events(database: &Database, collection: &str) -> Vec<Event> {
    let stream = format!("collection/{collection}");
    let filter = Filter {
        stream: &stream,
        after: 0,
        r#type: Some(RECORDED),
    };
    database
        .events(&ScopeSet::default_workspace(), &filter)
        .unwrap()
}

#[test]
fn a_report_is_stored_pinned_indexed_and_journaled() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let recorded = database
        .record_eval_report(&report("synthetic", SYNTHETIC))
        .unwrap();
    assert_eq!(recorded.collection_id, "synthetic");
    assert_eq!(recorded.generation, SYNTHETIC);
    assert_eq!(recorded.suite, "synthetic");
    assert_eq!(recorded.digest, Digest::of(JSON));
    assert!(
        recorded.recorded_at.len() == 24 && recorded.recorded_at.ends_with('Z'),
        "{}",
        recorded.recorded_at
    );
    assert_eq!(database.get(&recorded.digest).unwrap(), JSON);
    let artifact = database.artifact(&recorded.digest).unwrap().unwrap();
    assert_eq!(
        (artifact.pins, artifact.media.as_str()),
        (1, "application/json")
    );
    let scopes = ScopeSet::default_workspace();
    assert_eq!(
        database.eval_report(&scopes, recorded.id).unwrap(),
        Some(recorded.clone())
    );
    let events = recorded_events(&database, "synthetic");
    let [event] = events.as_slice() else {
        panic!("one event: {events:?}");
    };
    assert_eq!(event.r#type, "maestro.eval.report.recorded.v1");
    assert_eq!(event.subject, format!("eval-report/{}", recorded.id));
    assert_eq!(event.scope, "workspace/default/collection/synthetic");
    assert_eq!(
        event.data,
        json!({
            "report": recorded.id.to_string(), "collection": "synthetic",
            "generation": SYNTHETIC, "suite": "synthetic",
        })
    );
}

#[test]
fn each_report_recorded_is_a_record_of_its_own_listed_in_record_order() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let first = database
        .record_eval_report(&report("synthetic", SYNTHETIC))
        .unwrap();
    let second = database
        .record_eval_report(&report("synthetic", SYNTHETIC))
        .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(first.digest, second.digest);
    assert_eq!(database.artifact(&first.digest).unwrap().unwrap().pins, 2);
    assert_eq!(
        database
            .eval_reports(&ScopeSet::default_workspace(), "synthetic")
            .unwrap(),
        [first, second]
    );
    assert_eq!(recorded_events(&database, "synthetic").len(), 2);
}

#[test]
fn a_report_of_a_generation_its_collection_does_not_have_is_refused_and_records_nothing() {
    let scratch = Scratch::new();
    let database = scratch.open();
    for generation in [CTM, 99] {
        let error = database
            .record_eval_report(&report("synthetic", generation))
            .unwrap_err();
        assert!(
            matches!(&error, Error::UnknownGeneration { collection, generation: refused }
                if collection == "synthetic" && *refused == generation),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            format!("the collection synthetic records no generation {generation}")
        );
    }
    let scopes = ScopeSet::default_workspace();
    assert_eq!(ids(&database, &scopes, "synthetic"), Vec::<String>::new());
    assert_eq!(recorded_events(&database, "synthetic"), []);
    assert_eq!(
        database.artifact(&Digest::of(JSON)).unwrap().unwrap().pins,
        0
    );
}

#[test]
fn a_journal_that_refuses_the_event_leaves_no_report_and_no_pin() {
    let scratch = Scratch::new();
    let database = scratch.open();
    execute(
        &database,
        "CREATE TRIGGER refuse_events BEFORE INSERT ON events
         BEGIN SELECT RAISE(ABORT, 'no event today'); END",
    )
    .unwrap();
    let error = database
        .record_eval_report(&report("synthetic", SYNTHETIC))
        .unwrap_err();
    assert!(
        matches!(&error, Error::Store(store::Error::Sqlite(_))),
        "{error:?}"
    );
    assert_eq!(
        error.to_string(),
        "the kernel database refused the operation"
    );
    let cause = error::Error::source(&error).unwrap().to_string();
    assert!(cause.contains("no event today"), "{cause}");
    let scopes = ScopeSet::default_workspace();
    assert_eq!(ids(&database, &scopes, "synthetic"), Vec::<String>::new());
    assert_eq!(
        database.artifact(&Digest::of(JSON)).unwrap().unwrap().pins,
        0
    );
}

#[test]
fn reports_are_read_only_through_scopes_that_cover_their_collection() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let ctm = database.record_eval_report(&report("ctm", CTM)).unwrap();
    let synthetic = database
        .record_eval_report(&report("synthetic", SYNTHETIC))
        .unwrap();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    database
        .grant("reader", &scope, Right::Read, "test")
        .unwrap();
    let reader = database.visible("reader").unwrap();
    assert_eq!(
        ids(&database, &reader, "synthetic"),
        [synthetic.id.to_string()]
    );
    assert_eq!(ids(&database, &reader, "ctm"), Vec::<String>::new());
    assert_eq!(database.eval_report(&reader, ctm.id).unwrap(), None);
    assert_eq!(
        database.eval_report(&reader, synthetic.id).unwrap(),
        Some(synthetic.clone())
    );
    let nobody = database.visible("nobody").unwrap();
    assert_eq!(ids(&database, &nobody, "synthetic"), Vec::<String>::new());
    assert_eq!(database.eval_report(&nobody, synthetic.id).unwrap(), None);
    let everything = ScopeSet::default_workspace();
    assert_eq!(ids(&database, &everything, "ctm"), [ctm.id.to_string()]);
}
