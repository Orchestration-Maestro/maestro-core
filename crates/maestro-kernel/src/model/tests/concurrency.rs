//! Concurrent model registry writes stay idempotent.

use super::support::{Scratch, card, collection, collection_scopes};
use crate::{gateway::Role, model::NewModelCard};
use std::{sync::Barrier, thread};

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
