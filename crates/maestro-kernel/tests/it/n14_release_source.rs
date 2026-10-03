//! Manual sync relinquishes only its current lease, never the job or frontier.
use super::n04_frontier_support::{Scratch, dispatch, item, now, scopes, writer};
use maestro_kernel::{
    acquisition::{Error, Frontier},
    job::JobState,
    store::Database,
};
use rusqlite::Connection;
use std::time::Duration;

#[test]
fn n14_release_preserves_state_items_attempts_and_journal_then_releases_immediately() {
    let scratch = Scratch::new();
    let db = Database::open_in(&scratch).unwrap();
    let scopes = scopes(&db);
    let current = writer(&db, now());
    let item = db.enqueue(&current, &item(), now()).unwrap();
    db.lease(&current, item.id, dispatch(now())).unwrap();
    let before = Frontier::page(&db, &scopes, "docs", None, 1000).unwrap();
    let connection = Connection::open(scratch.join("kernel.sqlite3")).unwrap();
    let events: i64 = connection
        .query_row("SELECT count(*) FROM events", [], |row| row.get(0))
        .unwrap();
    db.release_source(&current, now()).unwrap();
    assert_eq!(
        Frontier::page(&db, &scopes, "docs", None, 1000).unwrap(),
        before
    );
    assert_eq!(
        db.job(&scopes, current.token).unwrap().unwrap().state,
        JobState::Running
    );
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM events", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        events
    );
    let successor = writer(&db, now());
    assert_eq!(successor.epoch, current.epoch + 1);
    assert!(db.enqueue(&successor, &item.request, now()).is_ok());
}

#[test]
fn n14_stale_writer_cannot_release_successor() {
    let scratch = Scratch::new();
    let db = Database::open_in(&scratch).unwrap();
    let current = writer(&db, now());
    let later = now() + Duration::from_secs(31);
    let successor = writer(&db, later);
    assert!(matches!(
        db.release_source(&current, later),
        Err(Error::Lost)
    ));
    assert!(db.enqueue(&successor, &item(), later).is_ok());
}
