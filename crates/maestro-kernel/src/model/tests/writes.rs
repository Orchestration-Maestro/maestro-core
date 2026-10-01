//! Transactional model-registry write preconditions and rollback tests.

use super::support::{
    Scratch, card, collection, collection_scopes, corrupt_artifact, generation_in, identity,
    new_eval,
};
use crate::{
    artifact::Digest,
    gateway::{
        ModelCard,
        card_v2::{Capability, TextFormat},
    },
    model::{Error as ModelError, EvaluationDisposition, EvaluationMode, NewModelCard},
    store::Error as StoreError,
};

#[test]
fn missing_collection_and_blank_run_id_fail_before_artifacts_are_recorded() {
    let scratch = Scratch::new();
    let database = scratch.open();
    let candidate = card(&database, &scratch);
    let missing_scope = collection_scopes(&database, "missing");
    let error = database
        .record_model_card(
            &missing_scope,
            &NewModelCard {
                collection_id: "missing",
                card: &candidate,
            },
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Invalid(reason) if reason.contains("not registered")));
    assert!(database.artifact(candidate.digest()).unwrap().is_none());

    collection(&database, "one");
    let scopes = collection_scopes(&database, "one");
    let registration = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "one",
                card: &candidate,
            },
        )
        .unwrap();
    let report = b"blank-run report";
    let manifest = b"blank-run manifest";
    let error = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registration.id,
                "one",
                " ",
                EvaluationMode::Real,
                EvaluationDisposition::Failed,
                report,
                manifest,
            ),
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Invalid(reason) if reason == "run_id is blank"));
    assert!(database.artifact(&Digest::of(report)).unwrap().is_none());
    assert!(database.artifact(&Digest::of(manifest)).unwrap().is_none());
}

#[test]
fn card_registration_ignores_an_unrelated_damaged_card() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    let scopes = collection_scopes(&database, "one");
    let mut damaged_identity = identity(&database);
    damaged_identity.formats.query = Capability::Supported(TextFormat {
        prefix: "damaged".to_owned(),
        suffix: String::new(),
    });
    let damaged_card = ModelCard::record_v2(&scratch.store(), &damaged_identity).unwrap();
    let damaged = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "one",
                card: &damaged_card,
            },
        )
        .unwrap();
    corrupt_artifact(&scratch, &damaged.digest);

    let mut next_identity = identity(&database);
    next_identity.formats.query = Capability::Supported(TextFormat {
        prefix: "unrelated".to_owned(),
        suffix: String::new(),
    });
    let next_card = ModelCard::record_v2(&scratch.store(), &next_identity).unwrap();
    let next = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "one",
                card: &next_card,
            },
        )
        .unwrap();
    assert_ne!(next.id, damaged.id);
}

#[test]
fn selection_reads_only_its_exact_evaluation() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    let scopes = collection_scopes(&database, "one");
    let candidate = card(&database, &scratch);
    let registration = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "one",
                card: &candidate,
            },
        )
        .unwrap();
    let unrelated = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registration.id,
                "one",
                "unrelated",
                EvaluationMode::Real,
                EvaluationDisposition::Failed,
                b"unrelated report",
                b"unrelated manifest",
            ),
        )
        .unwrap();
    let chosen = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registration.id,
                "one",
                "chosen",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                b"chosen report",
                b"chosen manifest",
            ),
        )
        .unwrap();
    corrupt_artifact(&scratch, &unrelated.report_digest);
    assert!(matches!(
        database.model_evaluations(&scopes, "one"),
        Err(ModelError::Store(_))
    ));
    let selection = database
        .record_model_selection(
            &scopes,
            &crate::model::NewModelSelection {
                collection_id: "one",
                role: registration.role,
                card_id: registration.id,
                evaluation_id: chosen.id,
                selected_by: "owner",
                reason: "specific evaluation",
            },
        )
        .unwrap();
    assert_eq!(selection.evaluation_id, chosen.id);
}

#[test]
fn evaluation_checks_generation_scope_and_does_not_keep_generation_alive() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    collection(&database, "two");
    let scopes = collection_scopes(&database, "one");
    let candidate = card(&database, &scratch);
    let registration = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "one",
                card: &candidate,
            },
        )
        .unwrap();
    let own_generation = generation_in(&database, "one");
    let foreign_generation = generation_in(&database, "two");

    let foreign_report = b"foreign-generation report";
    let foreign_manifest = b"foreign-generation manifest";
    let mut foreign = new_eval(
        registration.id,
        "one",
        "foreign-generation",
        EvaluationMode::Real,
        EvaluationDisposition::Failed,
        foreign_report,
        foreign_manifest,
    );
    foreign.generation_id = Some(foreign_generation);
    let error = database
        .record_model_evaluation(&scopes, &foreign)
        .unwrap_err();
    assert!(matches!(error, ModelError::Invalid(_)));
    assert!(
        database
            .artifact(&Digest::of(foreign_report))
            .unwrap()
            .is_none()
    );
    assert!(
        database
            .artifact(&Digest::of(foreign_manifest))
            .unwrap()
            .is_none()
    );

    let report = b"retired-generation report";
    let manifest = b"retired-generation manifest";
    let mut evaluation = new_eval(
        registration.id,
        "one",
        "retired-generation",
        EvaluationMode::Real,
        EvaluationDisposition::Eligible,
        report,
        manifest,
    );
    evaluation.generation_id = Some(own_generation);
    let recorded = database
        .record_model_evaluation(&scopes, &evaluation)
        .unwrap();
    assert_eq!(recorded.generation_id, Some(own_generation));
    database
        .write(|transaction| {
            transaction.execute("DELETE FROM generations WHERE id=?1", [own_generation])?;
            Ok::<_, StoreError>(())
        })
        .unwrap();
    assert!(
        database
            .model_evaluations(&scopes, "one")
            .unwrap()
            .iter()
            .any(|record| record.generation_id == Some(own_generation))
    );
}
