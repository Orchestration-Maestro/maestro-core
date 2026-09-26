//! Quality dispositions: one per revision, kept once given, read only inside
//! the caller's scopes, and journaled in the write that records them when
//! they hold a revision back.

use super::support::{Scratch, revision};
use crate::{
    document::{Disposition, Error, Outcome, Recorded, Revision, RevisionStatus},
    journal::{Event, Filter},
    scope::ScopeSet,
    store::{self, Database},
};
use serde_json::{Value, json};

/// Every outcome, with the name the `disposition` column holds for it.
const OUTCOMES: [(Outcome, &str); 5] = [
    (Outcome::Accepted, "accepted"),
    (Outcome::AcceptedWithWarnings, "accepted_with_warnings"),
    (Outcome::NeedsReextraction, "needs_reextraction"),
    (Outcome::Quarantined, "quarantined"),
    (Outcome::Excluded, "excluded"),
];

/// The disposition `outcome` of the revision `revision_id`, as the import
/// decides it.
fn disposition(revision_id: &str, outcome: Outcome) -> Disposition {
    Disposition {
        revision_id: revision_id.to_owned(),
        outcome,
        reasons: vec![format!("{revision_id} shares its source_ref")],
        rule_ids: vec!["import.shared-source-ref".to_owned()],
        decided_by: "import".to_owned(),
    }
}

/// The database of `scratch`, with the valid revisions `ids` of `doc-a`.
fn with_revisions(scratch: &Scratch, ids: &[&str]) -> Database {
    let database = scratch.open();
    for id in ids {
        let given = revision(&database, id, RevisionStatus::Valid);
        database.record_revision(&given).unwrap();
    }
    database
}

/// The disposition of `revision_id`, as the whole default workspace reads
/// it.
fn read(database: &Database, revision_id: &str) -> Option<Disposition> {
    database
        .disposition(&ScopeSet::default_workspace(), revision_id)
        .unwrap()
}

/// The dispositions the table holds, whatever their scope.
fn rows(database: &Database) -> Vec<(String, String)> {
    let reader = database.reader().unwrap();
    let mut statement = reader
        .prepare("SELECT revision_id, disposition FROM quality_dispositions ORDER BY revision_id")
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// Every event of the stream of the collection `ctm`.
fn events(database: &Database) -> Vec<Event> {
    let filter = Filter {
        stream: "collection/ctm",
        after: 0,
        r#type: None,
    };
    database
        .events(&ScopeSet::default_workspace(), &filter)
        .unwrap()
}

/// The data of the event of the revision `revision` of `ctm` held as
/// `disposition`.
fn held(revision: &str, disposition: &str) -> Value {
    json!({ "collection": "ctm", "revision": revision, "disposition": disposition })
}

#[test]
fn a_disposition_is_recorded_once_and_read_back_whole() {
    let scratch = Scratch::new();
    let database = with_revisions(&scratch, &["rev-a"]);
    let given = disposition("rev-a", Outcome::Quarantined);
    assert_eq!(database.record_disposition(&given).unwrap(), Recorded::New);
    assert_eq!(read(&database, "rev-a"), Some(given));
    assert_eq!(read(&database, "rev-b"), None);
}

#[test]
fn a_revision_keeps_the_first_disposition_it_was_given() {
    let scratch = Scratch::new();
    let database = with_revisions(&scratch, &["rev-a"]);
    let first = disposition("rev-a", Outcome::Quarantined);
    database.record_disposition(&first).unwrap();
    let later = Disposition {
        reasons: vec!["every check passed".to_owned()],
        rule_ids: Vec::new(),
        decided_by: "quality".to_owned(),
        ..disposition("rev-a", Outcome::Accepted)
    };
    assert_eq!(
        database.record_disposition(&later).unwrap(),
        Recorded::Unchanged
    );
    assert_eq!(
        database.record_disposition(&first).unwrap(),
        Recorded::Unchanged
    );
    assert_eq!(read(&database, "rev-a"), Some(first));
    assert_eq!(
        events(&database).len(),
        1,
        "a kept disposition journals nothing"
    );
}

#[test]
fn each_outcome_is_recorded_by_name_and_only_the_three_holds_are_journaled() {
    let ids = ["rev-a", "rev-b", "rev-c", "rev-d", "rev-e"];
    let scratch = Scratch::new();
    let database = with_revisions(&scratch, &ids);
    for (id, (outcome, _)) in ids.into_iter().zip(OUTCOMES) {
        let recorded = database.record_disposition(&disposition(id, outcome));
        assert_eq!(recorded.unwrap(), Recorded::New, "{outcome:?}");
    }
    let named = ids
        .into_iter()
        .zip(OUTCOMES)
        .map(|(id, (_, name))| (id.to_owned(), name.to_owned()));
    assert_eq!(rows(&database), named.collect::<Vec<_>>());
    for (id, (outcome, _)) in ids.into_iter().zip(OUTCOMES) {
        assert_eq!(read(&database, id), Some(disposition(id, outcome)));
    }
    let journaled: Vec<(String, Value)> = events(&database)
        .into_iter()
        .map(|event| (event.subject, event.data))
        .collect();
    assert_eq!(
        journaled,
        [
            ("revision/rev-c", held("rev-c", "needs_reextraction")),
            ("revision/rev-d", held("rev-d", "quarantined")),
            ("revision/rev-e", held("rev-e", "excluded")),
        ]
        .map(|(subject, data)| (subject.to_owned(), data))
    );
}

#[test]
fn a_hold_is_journaled_on_its_collection_stream_in_its_source_scope() {
    let scratch = Scratch::new();
    let database = with_revisions(&scratch, &["rev-a"]);
    database
        .record_disposition(&disposition("rev-a", Outcome::Quarantined))
        .unwrap();
    let journaled = events(&database);
    let [event] = journaled.as_slice() else {
        panic!("one event: {journaled:#?}");
    };
    assert_eq!(event.stream, "collection/ctm");
    assert_eq!(event.sequence, 1);
    assert_eq!(event.r#type, "maestro.knowledge.revision.held.v1");
    assert_eq!(event.subject, "revision/rev-a");
    assert_eq!(event.scope, "workspace/default/collection/ctm/source/docs");
    assert_eq!(event.data, held("rev-a", "quarantined"));
}

#[test]
fn a_disposition_is_read_only_inside_the_scopes_that_cover_its_source() {
    let scratch = Scratch::new();
    let database = with_revisions(&scratch, &["rev-a"]);
    let given = disposition("rev-a", Outcome::Excluded);
    database.record_disposition(&given).unwrap();
    let set =
        |paths: &[&str]| ScopeSet::new(paths.iter().map(|path| path.parse().unwrap()).collect());
    for inside in [
        "workspace/default/collection/ctm",
        "workspace/default/collection/ctm/source/docs",
    ] {
        let read = database.disposition(&set(&[inside]), "rev-a").unwrap();
        assert_eq!(read.as_ref(), Some(&given), "{inside}");
    }
    for outside in [
        &[][..],
        &["workspace/default/collection/ctm-archive"],
        &["workspace/default/collection/ctm/source/doc"],
        &["workspace/other"],
    ] {
        let read = database.disposition(&set(outside), "rev-a").unwrap();
        assert_eq!(read, None, "{outside:?}");
    }
}

#[test]
fn a_disposition_of_a_revision_never_recorded_is_refused_and_journals_nothing() {
    let scratch = Scratch::new();
    let database = with_revisions(&scratch, &[]);
    let refused = database.record_disposition(&disposition("rev-z", Outcome::Quarantined));
    assert!(matches!(refused, Err(Error::Store(_))), "{refused:?}");
    assert!(rows(&database).is_empty());
    assert!(events(&database).is_empty());
}

#[test]
fn a_hold_whose_event_the_journal_refuses_is_not_recorded_either() {
    let scratch = Scratch::new();
    let database = scratch.open();
    // A source id that is not a scope name, which only a program outside the
    // kernel could record: its scope path is refused by the journal.
    database
        .write(|transaction| {
            transaction.execute_batch(
                "INSERT INTO sources (collection_id, id, kind, reference, profiles_json)
                 VALUES ('ctm', 'Docs', 'import', 'corpus_root:Docs.jsonl', '{}');
                 INSERT INTO documents (id, collection_id, source_id, source_ref)
                 VALUES ('doc-b', 'ctm', 'Docs', 'https://example.org/b');",
            )?;
            Ok::<_, store::Error>(())
        })
        .unwrap();
    let given = Revision {
        document_id: "doc-b".to_owned(),
        ..revision(&database, "rev-b", RevisionStatus::Valid)
    };
    database.record_revision(&given).unwrap();
    let refused = database.record_disposition(&disposition("rev-b", Outcome::Quarantined));
    assert!(matches!(refused, Err(Error::Store(_))), "{refused:?}");
    assert!(
        rows(&database).is_empty(),
        "the hold rolled back with its event"
    );
    let accepted = database.record_disposition(&disposition("rev-b", Outcome::Accepted));
    assert_eq!(
        accepted.unwrap(),
        Recorded::New,
        "an admission journals nothing"
    );
}
