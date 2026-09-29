//! Concurrent model registry writes stay idempotent.

use super::support::{Scratch, card, collection, collection_scopes};
use crate::{gateway::Role, model::NewModelCard, store::Error as StoreError};
use rusqlite::params;
use std::{
    sync::{Barrier, mpsc},
    thread,
    time::Duration,
};
use ulid::Ulid;

#[test]
fn simultaneous_registration_of_one_card_records_one_immutable_card() {
    let scratch = Scratch::new();
    let databases = (0..16).map(|_| scratch.open()).collect::<Vec<_>>();
    collection(&databases[0], "one");
    let scopes = collection_scopes(&databases[0], "one");
    let card = card(&databases[0], &scratch);
    let new = NewModelCard {
        collection_id: "one",
        card: &card,
    };
    let start = Barrier::new(16);
    let registrations = thread::scope(|workers| {
        let workers = databases
            .iter()
            .map(|database| {
                let start = &start;
                let scopes = &scopes;
                let new = &new;
                workers.spawn(move || {
                    start.wait();
                    database.record_model_card(scopes, new).unwrap()
                })
            })
            .collect::<Vec<_>>();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert!(registrations.windows(2).all(|pair| pair[0] == pair[1]));
    let records = databases[0]
        .model_cards(&scopes, "one", Role::Embedder)
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0], registrations[0]);
    assert_eq!(
        databases[0]
            .artifact(&registrations[0].digest)
            .unwrap()
            .unwrap()
            .pins,
        1
    );
}

#[test]
fn a_registration_that_loses_the_race_returns_the_committed_identical_card() {
    let scratch = Scratch::new();
    let writer = scratch.open();
    let registrant = scratch.open();
    collection(&writer, "one");
    let scopes = collection_scopes(&writer, "one");
    let card = card(&writer, &scratch);
    let json = String::from_utf8(card.card_json().unwrap()).unwrap();
    let id = Ulid::generate();
    let (inserted, started) = mpsc::channel();
    let (card, scopes, registrant) = (&card, &scopes, &registrant);
    let registered = thread::scope(|workers| {
        let registration = workers.spawn(move || {
            // Bounded: a writer that fails before its insert fails the test
            // instead of leaving this thread waiting forever.
            started.recv_timeout(Duration::from_secs(10)).unwrap();
            registrant.record_model_card(
                scopes,
                &NewModelCard {
                    collection_id: "one",
                    card,
                },
            )
        });
        writer
            .write(|transaction| {
                transaction.execute(
                    "INSERT INTO model_cards (id, collection_id, role, digest, card_json)
                     VALUES (?1, 'one', ?2, ?3, ?4)",
                    params![
                        id.to_string(),
                        card.fields().role.to_string(),
                        card.digest().as_str(),
                        json
                    ],
                )?;
                // The registrant reads before this commit, then waits for the write lock.
                inserted.send(()).unwrap();
                thread::sleep(Duration::from_millis(200));
                Ok::<(), StoreError>(())
            })
            .unwrap();
        registration.join().unwrap()
    });
    assert_eq!(registered.unwrap().id, id);
}
