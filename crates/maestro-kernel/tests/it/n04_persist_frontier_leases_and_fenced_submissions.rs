//! N04 frontier contract: durable work, exclusive leases and fenced acknowledgement.
#![cfg(test)]

use super::n04_frontier_support::{Scratch, dispatch, item, now, request, scopes, writer};
use maestro_kernel::{
    acquisition::{Error, Frontier},
    artifact::Digest,
    scope::Scope,
    store::Database,
};
use std::{
    sync::Barrier,
    thread,
    time::{Duration, Instant},
};

#[test]
fn n04_two_source_writers_race_on_independent_connections() {
    let root = Scratch::new();
    let first_db = Database::open_in(&root).unwrap();
    let second_db = Database::open_in(&root).unwrap();
    let barrier = Barrier::new(2);
    let race = |frontier: &dyn Frontier| {
        barrier.wait();
        frontier.lease_source(
            "docs",
            &"workspace/default/collection/docs".parse().unwrap(),
            request(now()),
        )
    };
    thread::scope(|threads| {
        let first = threads.spawn(|| race(&first_db));
        let second = threads.spawn(|| race(&second_db));
        let results = [first.join().unwrap(), second.join().unwrap()];
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(results.iter().filter(|result| result.is_err()).count(), 1);
        assert_eq!(results.into_iter().find_map(Result::ok).unwrap().epoch, 1);
    });
}

#[test]
fn n04_equivalent_requests_race_but_contexts_never_coalesce() {
    let root = Scratch::new();
    let first_db = Database::open_in(&root).unwrap();
    let second_db = Database::open_in(&root).unwrap();
    let source = writer(&first_db, now());
    let barrier = Barrier::new(2);
    let race = |frontier: &dyn Frontier| {
        let row = frontier.enqueue(&source, &item(), now()).unwrap();
        barrier.wait();
        (row.id, frontier.lease(&source, row.id, dispatch(now())))
    };
    thread::scope(|threads| {
        let first = threads.spawn(|| race(&first_db));
        let second = threads.spawn(|| race(&second_db));
        let results = [first.join().unwrap(), second.join().unwrap()];
        assert_eq!(results[0].0, results[1].0);
        assert_eq!(
            results.iter().filter(|(_, result)| result.is_ok()).count(),
            1
        );
    });
    let frontier: &dyn Frontier = &first_db;
    let mut auth = item();
    auth.authorization_context = Digest::of(b"account-b");
    let mut repr = item();
    repr.representation_profile = Digest::of(b"rendered-dom");
    let original = frontier.page(&scopes(&first_db), "docs", None, 1).unwrap()[0].id;
    for separate in [auth, repr] {
        let row = frontier.enqueue(&source, &separate, now()).unwrap();
        assert_ne!(row.id, original);
        assert_eq!(row.request, separate);
        frontier.lease(&source, row.id, dispatch(now())).unwrap();
    }
    let replay = frontier.enqueue(&source, &item(), now()).unwrap();
    assert_eq!(replay.id, original);
    assert_eq!(replay.request, item());
    let rows = frontier.page(&scopes(&first_db), "docs", None, 10).unwrap();
    assert_eq!(rows.len(), 3);
    assert!(
        rows.iter()
            .all(|result| result.attempts == 1 && result.capture.is_none())
    );
}

#[test]
fn n04_expired_source_refuses_before_takeover() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &db;
    let source = writer(frontier, now());
    let row = frontier.enqueue(&source, &item(), now()).unwrap();
    let mut long = dispatch(now());
    long.lease.term = Duration::from_secs(60);
    let lease = frontier.lease(&source, row.id, long).unwrap();
    let digest = db.put(b"capture", "text/plain").unwrap();
    let expiry = now() + Duration::from_secs(30);
    assert!(
        frontier
            .acknowledge(&source, &lease, &digest, expiry)
            .is_err()
    );
    assert!(frontier.enqueue(&source, &item(), expiry).is_err());
    assert!(frontier.lease(&source, row.id, dispatch(expiry)).is_err());
    assert_eq!(
        frontier.page(&scopes(&db), "docs", None, 1).unwrap()[0].capture,
        None
    );
    assert_eq!(db.artifact(&digest).unwrap().unwrap().pins, 0);
}

#[test]
fn n04_expired_item_refuses_and_retries_with_higher_epoch() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &db;
    let source = writer(frontier, now());
    let row = frontier.enqueue(&source, &item(), now()).unwrap();
    let mut short = dispatch(now());
    short.lease.term = Duration::from_secs(5);
    let old = frontier.lease(&source, row.id, short).unwrap();
    let digest = db.put(b"capture", "text/plain").unwrap();
    let later = now() + Duration::from_secs(5);
    assert!(frontier.acknowledge(&source, &old, &digest, later).is_err());
    let current = frontier.lease(&source, row.id, dispatch(later)).unwrap();
    assert_eq!(current.epoch, 2);
    assert!(frontier.acknowledge(&source, &old, &digest, later).is_err());
    frontier
        .acknowledge(&source, &current, &digest, later)
        .unwrap();
    frontier
        .acknowledge(&source, &current, &digest, later)
        .unwrap();
    let rows = frontier.page(&scopes(&db), "docs", None, 10).unwrap();
    assert_eq!(rows[0].attempts, 2);
    assert_eq!(rows[0].capture, Some(digest.clone()));
    assert_eq!(db.artifact(&digest).unwrap().unwrap().pins, 1);
    assert!(matches!(
        frontier.lease(&source, row.id, dispatch(later)),
        Err(Error::Unavailable)
    ));
    let other = db.put(b"substitution", "text/plain").unwrap();
    assert!(
        frontier
            .acknowledge(&source, &current, &other, later)
            .is_err()
    );
    let takeover_time = now() + Duration::from_secs(30);
    let next_writer = writer(frontier, takeover_time);
    assert!(matches!(
        frontier.lease(&next_writer, row.id, dispatch(takeover_time)),
        Err(Error::Unavailable)
    ));
}

#[test]
fn n04_restart_preserves_pending_attempts_and_fences_previous_writer() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &db;
    let source = writer(frontier, now());
    let pending = frontier.enqueue(&source, &item(), now()).unwrap();
    let mut next = item();
    next.fetch_identity.push_str("/next");
    let in_flight = frontier.enqueue(&source, &next, now()).unwrap();
    let mut long = dispatch(now());
    long.lease.term = Duration::from_secs(60);
    let old = frontier.lease(&source, in_flight.id, long).unwrap();
    let digest = db.put(b"capture", "text/plain").unwrap();
    drop(db);
    let resumed = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &resumed;
    let later = now() + Duration::from_secs(30);
    let current = writer(frontier, later);
    assert_eq!(current.epoch, 2);
    assert_eq!(current.token, source.token);
    assert!(frontier.acknowledge(&source, &old, &digest, later).is_err());
    assert!(
        frontier
            .acknowledge(&current, &old, &digest, later)
            .is_err()
    );
    let mut retagged = old.clone();
    retagged.source_epoch = current.epoch;
    assert!(
        frontier
            .acknowledge(&current, &retagged, &digest, later)
            .is_err()
    );
    let retry = frontier.lease(&current, in_flight.id, dispatch(later));
    assert!(retry.is_ok());
    let retry = retry.unwrap();
    assert_eq!(retry.epoch, 2);
    frontier
        .acknowledge(&current, &retry, &digest, later)
        .unwrap();
    let rows = frontier.page(&scopes(&resumed), "docs", None, 10).unwrap();
    let pending = rows.iter().find(|result| result.id == pending.id).unwrap();
    assert_eq!(pending.attempts, 0);
    assert_eq!(pending.capture, None);
    assert_eq!(
        rows.iter()
            .find(|result| result.id == in_flight.id)
            .unwrap()
            .attempts,
        2
    );
}

#[test]
fn n04_invalid_bounds_and_identities_refuse_without_work() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &db;
    let scope: Scope = "workspace/default/collection/docs".parse().unwrap();
    for invalid in ["", "../docs", "docs/other", "-docs", &"a".repeat(129)] {
        assert!(
            frontier
                .lease_source(invalid, &scope, request(now()))
                .is_err()
        );
    }
    for term in [
        Duration::ZERO,
        Duration::from_nanos(1),
        Duration::from_secs(3601),
    ] {
        let mut result = request(now());
        result.term = term;
        assert!(frontier.lease_source("docs", &scope, result).is_err());
    }
    let oversized = "a".repeat(129);
    for holder in ["", "nul\0holder", oversized.as_str()] {
        let mut invalid = request(now());
        invalid.holder = holder;
        assert!(frontier.lease_source("docs", &scope, invalid).is_err());
    }
    let mut maximum = request(now());
    let holder = "a".repeat(128);
    maximum.holder = &holder;
    maximum.term = Duration::from_hours(1);
    assert!(
        frontier
            .lease_source(&"a".repeat(128), &scope, maximum)
            .is_ok()
    );
    let mut minimum = request(now());
    minimum.term = Duration::from_millis(1);
    assert!(frontier.lease_source("minimum", &scope, minimum).is_ok());
    let source = writer(frontier, now());
    for invalid in [
        String::new(),
        "https://example.test/\0".into(),
        "x".repeat(8193),
    ] {
        let mut input = item();
        input.fetch_identity = invalid;
        assert!(frontier.enqueue(&source, &input, now()).is_err());
    }
    assert!(frontier.page(&scopes(&db), "docs", None, 0).is_err());
    assert!(frontier.page(&scopes(&db), "docs", None, 1001).is_err());
    assert!(
        frontier
            .page(&scopes(&db), "docs", None, 10)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn n04_scope_paging_and_wrong_source_handles_refuse() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &db;
    let source = writer(frontier, now());
    let row = frontier.enqueue(&source, &item(), now()).unwrap();
    let mut other_item = item();
    other_item.fetch_identity.push_str("/2");
    frontier.enqueue(&source, &other_item, now()).unwrap();
    let first = frontier.page(&scopes(&db), "docs", None, 1).unwrap();
    let second = frontier
        .page(&scopes(&db), "docs", Some(first[0].id), 1)
        .unwrap();
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 1);
    assert_ne!(first[0].id, second[0].id);
    assert!(
        frontier
            .page(&scopes(&db), "docs", Some(second[0].id), 1)
            .unwrap()
            .is_empty()
    );
    let denied = db.visible("denied-reader").unwrap();
    assert!(frontier.page(&denied, "docs", None, 10).unwrap().is_empty());
    let other = frontier
        .lease_source(
            "other",
            &"workspace/default/collection/other".parse().unwrap(),
            request(now()),
        )
        .unwrap();
    assert!(frontier.lease(&other, row.id, dispatch(now())).is_err());
    let lease = frontier.lease(&source, row.id, dispatch(now())).unwrap();
    let digest = db.put(b"capture", "text/plain").unwrap();
    assert!(
        frontier
            .acknowledge(&other, &lease, &digest, now())
            .is_err()
    );
    frontier
        .acknowledge(&source, &lease, &digest, now())
        .unwrap();
}

#[test]
fn n04_forged_handles_and_monotonic_expiry_refuse() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &db;
    let source = writer(frontier, now());
    let row = frontier.enqueue(&source, &item(), now()).unwrap();
    let lease = frontier.lease(&source, row.id, dispatch(now())).unwrap();
    let digest = db.put(b"capture", "text/plain").unwrap();
    let other = frontier
        .lease_source(
            "other",
            &"workspace/default/collection/other".parse().unwrap(),
            request(now()),
        )
        .unwrap();
    let mut wrong_epoch = source.clone();
    wrong_epoch.epoch = 0;
    assert!(frontier.enqueue(&wrong_epoch, &item(), now()).is_err());
    let mut wrong_token = source.clone();
    wrong_token.token = other.token;
    assert!(frontier.enqueue(&wrong_token, &item(), now()).is_err());
    let mut wrong_epoch = lease.clone();
    wrong_epoch.source_epoch = 0;
    assert!(
        frontier
            .acknowledge(&source, &wrong_epoch, &digest, now())
            .is_err()
    );
    let mut wrong_item = lease.clone();
    wrong_item.epoch = 0;
    assert!(
        frontier
            .acknowledge(&source, &wrong_item, &digest, now())
            .is_err()
    );
    let mut forged = source.clone();
    forged.source = "other".into();
    assert!(frontier.enqueue(&forged, &item(), now()).is_err());
    forged = source.clone();
    forged.holder = "intruder".into();
    assert!(frontier.enqueue(&forged, &item(), now()).is_err());
    let mut expired = source.clone();
    expired.deadline = Instant::now();
    assert!(
        frontier
            .acknowledge(&expired, &lease, &digest, now())
            .is_err()
    );
    let mut expired_item = lease.clone();
    expired_item.deadline = Instant::now();
    assert!(
        frontier
            .acknowledge(&source, &expired_item, &digest, now())
            .is_err()
    );
    let mut wrong_holder = lease.clone();
    wrong_holder.holder = "intruder".into();
    assert!(
        frontier
            .acknowledge(&source, &wrong_holder, &digest, now())
            .is_err()
    );
    assert!(
        frontier
            .acknowledge(&source, &lease, &Digest::of(b"missing"), now())
            .is_err()
    );
    assert_eq!(db.artifact(&digest).unwrap().unwrap().pins, 0);
    frontier
        .acknowledge(&source, &lease, &digest, now())
        .unwrap();
}

#[test]
fn n04_attempt_ceiling_leaves_exhausted_work_visible() {
    let root = Scratch::new();
    let db = Database::open_in(&root).unwrap();
    let frontier: &dyn Frontier = &db;
    let source = writer(frontier, now());
    let row = frontier.enqueue(&source, &item(), now()).unwrap();
    let mut bounded = dispatch(now());
    bounded.max_attempts = 0;
    assert!(matches!(
        frontier.lease(&source, row.id, bounded),
        Err(Error::Invalid)
    ));
    bounded.max_attempts = 2;
    bounded.lease.term = Duration::from_secs(1);
    let first = frontier.lease(&source, row.id, bounded).unwrap();
    assert_eq!(first.epoch, 1);
    bounded.lease.now += Duration::from_secs(1);
    let last = frontier.lease(&source, row.id, bounded).unwrap();
    assert_eq!(last.epoch, 2);
    bounded.lease.now += Duration::from_secs(1);
    assert!(frontier.lease(&source, row.id, bounded).is_err());
    let rows = frontier.page(&scopes(&db), "docs", None, 10).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].attempts, 2);
    assert_eq!(rows[0].capture, None);
}
