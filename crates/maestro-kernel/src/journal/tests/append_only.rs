//! The journal is append-only: the database itself refuses to update, delete
//! or replace an event, whoever writes, and refuses what the journal never
//! stores.

use super::support::{Scratch, imported, whole};
use crate::store;
use serde_json::json;

/// Records one event, then runs `sql` on a connection outside the kernel and
/// returns SQLite's refusal, after checking the event is still as recorded.
fn refused(sql: &str) -> String {
    let scratch = Scratch::new();
    let database = scratch.open();
    let data = json!({"documents": 3});
    let event = database
        .record(&imported("collection/a", "import/1", &data))
        .unwrap();
    let outside = scratch.outside();
    let refusal = outside
        .execute_batch(&sql.replace("{id}", &event.id.to_string()))
        .unwrap_err();
    assert_eq!(whole(&database, "collection/a"), [event], "{sql}");
    refusal.to_string()
}

#[test]
fn an_event_is_never_updated() {
    let refusal = refused("UPDATE events SET data = '{}'");
    assert_eq!(
        refusal,
        "the journal is append-only: an event is never updated"
    );
}

#[test]
fn an_event_is_never_deleted() {
    let refusal = refused("DELETE FROM events");
    assert_eq!(
        refusal,
        "the journal is append-only: an event is never deleted"
    );
}

#[test]
fn an_event_is_never_replaced_by_an_insert() {
    let replaced = "the journal is append-only: an event is never replaced";
    let same_id = "INSERT OR REPLACE INTO events
                   (id, stream, sequence, type, subject, scope, data)
                   VALUES ('{id}', 'collection/b', 1, 'x', 'y', 'z', '{}')";
    assert_eq!(refused(same_id), replaced);
    let same_place = "INSERT OR REPLACE INTO events
                      (id, stream, sequence, type, subject, scope, data)
                      VALUES ('01ARZ3NDEKTSV4RRFFQ69G5FAV', 'collection/a', 1,
                              'x', 'y', 'z', '{}')";
    assert_eq!(refused(same_place), replaced);
    let upsert = "INSERT INTO events (id, stream, sequence, type, subject, scope, data)
                  VALUES ('{id}', 'collection/a', 1, 'x', 'y', 'z', '{}')
                  ON CONFLICT (id) DO UPDATE SET data = excluded.data";
    assert_eq!(refused(upsert), replaced);
}

#[test]
fn an_event_is_never_replaced_through_its_rowid() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let data = json!({"documents": 3});
    let event = database
        .record(&imported("collection/a", "import/1", &data))
        .unwrap();
    // Another ID in another stream passes the insert's trigger; the rowid is
    // the event's own.
    let replace = format!(
        "INSERT OR REPLACE INTO events (rowid, id, stream, sequence, type, subject, scope, data)
         SELECT rowid, '01ARZ3NDEKTSV4RRFFQ69G5FAV', 'collection/b', 1, 'x', 'y', 'z', '{{}}'
         FROM events WHERE id = '{}'",
        event.id
    );
    let refusal = database
        .write(|transaction| Ok::<_, store::Error>(transaction.execute(&replace, [])))
        .unwrap()
        .map_err(|error| error.to_string());
    assert_eq!(
        refusal,
        Err("the journal is append-only: an event is never deleted".to_owned())
    );
    assert_eq!(whole(&database, "collection/a"), [event]);
}

#[test]
fn the_tables_refuse_what_the_journal_never_stores() {
    let data = "INSERT INTO events (id, stream, sequence, type, subject, scope, data)
                VALUES ('01ARZ3NDEKTSV4RRFFQ69G5FAV', 'collection/b', 1,
                        'x', 'y', 'z', 'not json')";
    assert!(refused(data).contains("CHECK constraint failed"), "{data}");
    let sequence = "INSERT INTO events (id, stream, sequence, type, subject, scope, data)
                    VALUES ('01ARZ3NDEKTSV4RRFFQ69G5FAV', 'collection/b', 0,
                            'x', 'y', 'z', '{}')";
    assert!(
        refused(sequence).contains("CHECK constraint failed"),
        "{sequence}"
    );
    let position = "INSERT INTO cursors (consumer, stream, position)
                    VALUES ('indexer', 'collection/a', -1)";
    assert!(
        refused(position).contains("CHECK constraint failed"),
        "{position}"
    );
}
