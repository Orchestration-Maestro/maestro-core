//! N04 acknowledgement integrity, atomic journal writes and immutable source binding.
#![cfg(test)]
use super::n04_frontier_support::{Scratch, dispatch, item, now, request, scopes, writer};
use maestro_kernel::{
    acquisition::{Error, Frontier},
    job::{JobState, stream},
    journal::Filter,
    store::Database,
};
use std::{
    fs,
    sync::mpsc::{RecvTimeoutError, channel},
    thread,
    time::{Duration, Instant},
};

#[test]
fn n04_ack_rolls_back_pin_and_capture_when_journal_refuses() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &db;
    let source = writer(frontier, now());
    let row = frontier.enqueue(&source, &item(), now()).unwrap();
    frontier.enqueue(&source, &item(), now()).unwrap();
    let lease = frontier.lease(&source, row.id, dispatch(now())).unwrap();
    let digest = db.put(b"capture", "text/plain").unwrap();
    let outside = rusqlite::Connection::open(root.join("kernel.sqlite3")).unwrap();
    outside
        .execute_batch(
            "CREATE TRIGGER refuse_ack BEFORE INSERT ON events
        WHEN NEW.type = 'maestro.acquisition.acknowledged.v1'
        BEGIN SELECT RAISE(ABORT, 'synthetic journal failure'); END;",
        )
        .unwrap();
    assert!(
        frontier
            .acknowledge(&source, &lease, &digest, now())
            .is_err()
    );
    assert_eq!(db.artifact(&digest).unwrap().unwrap().pins, 0);
    assert_eq!(
        frontier.page(&scopes(&db), "docs", None, 1).unwrap()[0].capture,
        None
    );
    outside.execute_batch("DROP TRIGGER refuse_ack;").unwrap();
    frontier
        .acknowledge(&source, &lease, &digest, now())
        .unwrap();
    frontier
        .acknowledge(&source, &lease, &digest, now())
        .unwrap();
    let events = db
        .events(
            &scopes(&db),
            &Filter {
                stream: &stream(source.token),
                after: 0,
                r#type: None,
            },
        )
        .unwrap();
    assert_eq!(events.len(), 5);
    assert_eq!(events[2].r#type, "maestro.acquisition.enqueued.v1");
    assert_eq!(events[3].r#type, "maestro.acquisition.leased.v1");
    assert_eq!(events[4].r#type, "maestro.acquisition.acknowledged.v1");
    assert_eq!(events[4].data["item"], row.id.to_string());
    assert_eq!(db.artifact(&digest).unwrap().unwrap().pins, 1);
    assert!(
        outside
            .execute(
                "UPDATE acquisition_frontier SET capture = NULL WHERE id = ?1",
                [row.id.to_string()]
            )
            .is_err()
    );
}

#[test]
fn n04_ack_refuses_corrupt_artifact_and_keeps_work_pending() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &db;
    let source = writer(frontier, now());
    let row = frontier.enqueue(&source, &item(), now()).unwrap();
    let lease = frontier.lease(&source, row.id, dispatch(now())).unwrap();
    let digest = db.put(b"capture", "text/plain").unwrap();
    let hex = digest.as_str();
    let path = root
        .join("artifacts/sha256")
        .join(&hex[..2])
        .join(&hex[2..4])
        .join(hex);
    fs::write(path, b"corrupt").unwrap();
    assert!(
        frontier
            .acknowledge(&source, &lease, &digest, now())
            .is_err()
    );
    assert_eq!(
        frontier.page(&scopes(&db), "docs", None, 1).unwrap()[0].capture,
        None
    );
    assert_eq!(db.artifact(&digest).unwrap().unwrap().pins, 0);
    db.put(b"capture", "text/plain").unwrap();
    frontier
        .acknowledge(&source, &lease, &digest, now())
        .unwrap();
}

#[test]
fn n04_source_job_binding_cannot_be_replaced() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &db;
    let source = writer(frontier, now());
    let job = db.job(&scopes(&db), source.token).unwrap().unwrap();
    db.complete_job(
        &job.lease.unwrap(),
        JobState::Cancelled,
        &serde_json::json!({"reason": "synthetic external cancellation"}),
    )
    .unwrap();
    assert!(matches!(
        frontier.lease_source(
            "docs",
            &"workspace/default/collection/docs".parse().unwrap(),
            request(now())
        ),
        Err(Error::Lost)
    ));
    assert!(frontier.enqueue(&source, &item(), now()).is_err());
}

#[test]
fn n04_ack_rechecks_monotonic_item_deadline_after_waiting_for_writer() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &db;
    let source = writer(frontier, now());
    let row = frontier.enqueue(&source, &item(), now()).unwrap();
    let mut lease = frontier.lease(&source, row.id, dispatch(now())).unwrap();
    let digest = db.put(b"capture", "text/plain").unwrap();
    let outside = rusqlite::Connection::open(root.join("kernel.sqlite3")).unwrap();
    outside.execute_batch("BEGIN IMMEDIATE;").unwrap();
    lease.deadline = Instant::now() + Duration::from_millis(200);
    let (started, entered) = channel();
    let (finished, result) = channel();
    thread::scope(|threads| {
        let worker = threads.spawn(|| {
            started.send(()).unwrap();
            finished
                .send(frontier.acknowledge(&source, &lease, &digest, now()))
                .unwrap();
        });
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        // The acknowledgement cannot finish while SQLite's writer is held.
        assert!(matches!(
            result.recv_timeout(Duration::from_millis(400)),
            Err(RecvTimeoutError::Timeout)
        ));
        outside.execute_batch("ROLLBACK;").unwrap();
        assert!(
            result
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .is_err()
        );
        worker.join().unwrap();
    });
    assert_eq!(db.artifact(&digest).unwrap().unwrap().pins, 0);
    assert_eq!(
        frontier.page(&scopes(&db), "docs", None, 1).unwrap()[0].capture,
        None
    );
}

#[test]
fn n04_dispatch_refuses_expired_deadline_after_waiting_for_writer() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &db;
    let source = writer(frontier, now());
    let row = frontier.enqueue(&source, &item(), now()).unwrap();
    let outside = rusqlite::Connection::open(root.join("kernel.sqlite3")).unwrap();
    outside.execute_batch("BEGIN IMMEDIATE;").unwrap();
    let mut bounded = dispatch(now());
    bounded.lease.term = Duration::from_millis(200);
    bounded.max_attempts = 1;
    let (started, entered) = channel();
    let (finished, result) = channel();
    thread::scope(|threads| {
        let worker = threads.spawn(|| {
            started.send(()).unwrap();
            finished
                .send(frontier.lease(&source, row.id, bounded))
                .unwrap();
        });
        entered.recv_timeout(Duration::from_secs(5)).unwrap();
        // The dispatch cannot finish while SQLite's writer is held.
        assert!(matches!(
            result.recv_timeout(Duration::from_millis(400)),
            Err(RecvTimeoutError::Timeout)
        ));
        outside.execute_batch("ROLLBACK;").unwrap();
        let outcome = result.recv_timeout(Duration::from_secs(5)).unwrap();
        worker.join().unwrap();
        assert!(
            matches!(outcome, Err(Error::Lost)),
            "expired dispatch must refuse without consuming an attempt: {outcome:?}"
        );
    });
    let visible = scopes(&db);
    let pending = frontier.page(&visible, "docs", None, 1).unwrap();
    assert_eq!(pending[0].attempts, 0);
    assert_eq!(pending[0].epoch, 0);
    bounded.lease.term = Duration::from_secs(30);
    let granted = frontier.lease(&source, row.id, bounded).unwrap();
    assert!(granted.deadline > Instant::now());
    assert_eq!(granted.epoch, 1);
    let dispatched = frontier.page(&visible, "docs", None, 1).unwrap();
    assert_eq!(dispatched[0].attempts, 1);
    assert_eq!(dispatched[0].epoch, 1);
}
