//! Scope-bound visibility tests for model cards, evaluations, and selections.

use super::support::{
    Scratch, card, collection, collection_scopes, empty_scopes, grant, new_eval,
    populated_model_records,
};
use crate::{
    artifact::Digest,
    model::{
        Error as ModelError, EvaluationDisposition, EvaluationMode, NewModelCard, NewModelSelection,
    },
    scope::{Right, Scope},
};

#[test]
fn lists_and_lookups_hide_guessed_or_sibling_collection_records() {
    let scratch = Scratch::new();
    let (registration, evaluation, selection) = populated_model_records(&scratch);
    let database = scratch.open();
    collection(&database, "two");
    let digest = registration.digest.clone();
    let scopes_one = collection_scopes(&database, "one");
    let scopes_two = collection_scopes(&database, "two");

    assert_eq!(
        database
            .model_card(&scopes_one, "one", &digest)
            .unwrap()
            .unwrap()
            .digest(),
        &registration.digest
    );
    assert_eq!(
        database.model_evaluations(&scopes_one, "one").unwrap()[0].id,
        evaluation.id
    );
    assert_eq!(
        database
            .selected_model_card(&scopes_one, "one", registration.role)
            .unwrap()
            .unwrap()
            .selection,
        selection
    );

    assert!(
        database
            .model_card(&scopes_two, "one", &digest)
            .unwrap()
            .is_none()
    );
    assert!(
        database
            .model_cards(&scopes_two, "one", registration.role)
            .unwrap()
            .is_empty()
    );
    assert!(
        database
            .model_evaluations(&scopes_two, "one")
            .unwrap()
            .is_empty()
    );
    assert!(
        database
            .selected_model_card(&scopes_two, "one", registration.role)
            .unwrap()
            .is_none()
    );

    let empty = empty_scopes(&database);
    assert!(
        database
            .model_card(&empty, "one", &digest)
            .unwrap()
            .is_none()
    );
    assert!(
        database
            .model_cards(&empty, "one", registration.role)
            .unwrap()
            .is_empty()
    );
    assert!(
        database
            .model_evaluations(&empty, "one")
            .unwrap()
            .is_empty()
    );
    assert!(
        database
            .selected_model_card(&empty, "one", registration.role)
            .unwrap()
            .is_none()
    );

    let parent = grant(&database, "parent", "workspace/default");
    assert!(
        database
            .model_card(&parent, "one", &digest)
            .unwrap()
            .is_some()
    );
    assert_eq!(database.model_evaluations(&parent, "one").unwrap().len(), 1);
    assert_eq!(
        database
            .selected_model_card(&parent, "one", registration.role)
            .unwrap()
            .unwrap()
            .selection,
        selection
    );
}

#[test]
fn model_readers_refresh_after_collection_scope_revocation() {
    let scratch = Scratch::new();
    let (registration, _, _) = populated_model_records(&scratch);
    let database = scratch.open();
    let digest = registration.digest.clone();
    let scope: Scope = "workspace/default/collection/one".parse().unwrap();
    database
        .grant("revocable", &scope, Right::Read, "test")
        .unwrap();
    let granted = database.visible("revocable").unwrap();
    assert!(
        database
            .model_card(&granted, "one", &digest)
            .unwrap()
            .is_some()
    );
    assert_eq!(
        database.model_evaluations(&granted, "one").unwrap().len(),
        1
    );
    assert!(
        database
            .selected_model_card(&granted, "one", registration.role)
            .unwrap()
            .is_some()
    );

    database
        .revoke("revocable", &scope, Right::Read, "test")
        .unwrap();
    let revoked = database.visible("revocable").unwrap();
    assert!(
        database
            .model_card(&revoked, "one", &digest)
            .unwrap()
            .is_none()
    );
    assert!(
        database
            .model_evaluations(&revoked, "one")
            .unwrap()
            .is_empty()
    );
    assert!(
        database
            .selected_model_card(&revoked, "one", registration.role)
            .unwrap()
            .is_none()
    );
}

#[test]
fn identical_card_bytes_register_independently_in_two_collections() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    collection(&database, "two");
    let card = card(&database, &scratch);
    let digest = card.digest().clone();
    let scopes_one = collection_scopes(&database, "one");
    let one = database
        .record_model_card(
            &scopes_one,
            &NewModelCard {
                collection_id: "one",
                card: &card,
            },
        )
        .unwrap();
    let scopes_two = collection_scopes(&database, "two");
    let two = database
        .record_model_card(
            &scopes_two,
            &NewModelCard {
                collection_id: "two",
                card: &card,
            },
        )
        .unwrap();
    assert_eq!(one.digest, two.digest);
    assert_ne!(one.id, two.id);
    assert_eq!(database.artifact(&digest).unwrap().unwrap().pins, 2);
    assert!(
        database
            .model_card(&scopes_one, "two", &digest)
            .unwrap()
            .is_none()
    );
}

#[test]
fn unauthorized_model_card_registration_is_refused_before_side_effects() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    collection(&database, "two");
    let card = card(&database, &scratch);
    let sibling = collection_scopes(&database, "two");
    let empty = empty_scopes(&database);
    let digest = card.digest().clone();

    for scopes in [&sibling, &empty] {
        let error = database
            .record_model_card(
                scopes,
                &NewModelCard {
                    collection_id: "one",
                    card: &card,
                },
            )
            .unwrap_err();
        assert!(matches!(error, ModelError::Unauthorized));
        assert!(database.artifact(&digest).unwrap().is_none());
        assert_eq!(
            database
                .reader()
                .unwrap()
                .query_row("SELECT count(*) FROM model_cards", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}

#[test]
fn unauthorized_model_writes_are_refused_before_side_effects() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    collection(&database, "two");
    let card = card(&database, &scratch);
    let owner = collection_scopes(&database, "one");
    let sibling = collection_scopes(&database, "two");
    let registration = database
        .record_model_card(
            &owner,
            &NewModelCard {
                collection_id: "one",
                card: &card,
            },
        )
        .unwrap();

    let report = b"unauthorized report";
    let manifest = b"unauthorized manifest";
    let error = database
        .record_model_evaluation(
            &sibling,
            &new_eval(
                registration.id,
                "one",
                "unauthorized-run",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                report,
                manifest,
            ),
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Unauthorized));
    assert!(database.artifact(&Digest::of(report)).unwrap().is_none());
    assert!(database.artifact(&Digest::of(manifest)).unwrap().is_none());
    assert!(
        database
            .model_evaluations(&owner, "one")
            .unwrap()
            .is_empty()
    );

    let evaluation = database
        .record_model_evaluation(
            &owner,
            &new_eval(
                registration.id,
                "one",
                "authorized-run",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                b"authorized report",
                b"authorized manifest",
            ),
        )
        .unwrap();
    let error = database
        .record_model_selection(
            &sibling,
            &NewModelSelection {
                collection_id: "one",
                role: registration.role,
                card_id: registration.id,
                evaluation_id: evaluation.id,
                selected_by: "owner",
                reason: "unauthorized selection",
            },
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Unauthorized));
    assert!(
        database
            .selected_model_card(&owner, "one", registration.role)
            .unwrap()
            .is_none()
    );
    assert_eq!(
        database
            .reader()
            .unwrap()
            .query_row("SELECT count(*) FROM model_selections", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
