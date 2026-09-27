//! Transactional model-registry journal event contents and rollback.

use super::support::{Scratch, card, collection, collection_scopes, new_eval};
use crate::{
    artifact::Digest,
    journal::{self, Filter},
    model::{Error as ModelError, EvaluationDisposition, EvaluationMode, NewModelCard},
};

#[test]
fn card_and_evaluation_events_record_the_exact_registry_evidence() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "docs");
    let scopes = collection_scopes(&database, "docs");
    let candidate = card(&database, &scratch);
    let registration = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "docs",
                card: &candidate,
            },
        )
        .unwrap();
    let card_events = database
        .events(
            &scopes,
            &Filter {
                stream: &journal::stream("docs"),
                after: 0,
                r#type: Some("maestro.model.card.recorded.v1"),
            },
        )
        .unwrap();
    let card_event = card_events.first().unwrap();
    let registration_id = registration.id.to_string();
    assert_eq!(
        card_event.data["card"].as_str(),
        Some(registration_id.as_str())
    );
    assert_eq!(card_event.data["collection"].as_str(), Some("docs"));
    assert_eq!(card_event.data["role"].as_str(), Some("embedder"));
    assert_eq!(
        card_event.data["card_digest"].as_str(),
        Some(registration.digest.as_str())
    );

    let evaluation = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registration.id,
                "docs",
                "event-run",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                b"event report",
                b"event manifest",
            ),
        )
        .unwrap();
    let evaluation_events = database
        .events(
            &scopes,
            &Filter {
                stream: &journal::stream("docs"),
                after: 0,
                r#type: Some("maestro.model.evaluation.recorded.v1"),
            },
        )
        .unwrap();
    let evaluation_event = evaluation_events.first().unwrap();
    let evaluation_id = evaluation.id.to_string();
    assert_eq!(
        evaluation_event.data["evaluation"].as_str(),
        Some(evaluation_id.as_str())
    );
    assert_eq!(
        evaluation_event.data["card_digest"].as_str(),
        Some(registration.digest.as_str())
    );
    assert_eq!(
        evaluation_event.data["manifest_digest"].as_str(),
        Some(evaluation.manifest_digest.as_str())
    );
    assert_eq!(
        evaluation_event.data["report_digest"].as_str(),
        Some(evaluation.report_digest.as_str())
    );
}

#[test]
fn evaluation_journal_failure_rolls_back_its_record_and_pins() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "docs");
    let scopes = collection_scopes(&database, "docs");
    let candidate = card(&database, &scratch);
    let registration = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "docs",
                card: &candidate,
            },
        )
        .unwrap();
    scratch
        .outside()
        .execute_batch("DROP TABLE events;")
        .unwrap();
    let report = b"rolled-back report";
    let manifest = b"rolled-back manifest";
    let error = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registration.id,
                "docs",
                "journal-failure",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                report,
                manifest,
            ),
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Store(_)));
    assert_eq!(
        database
            .reader()
            .unwrap()
            .query_row("SELECT count(*) FROM model_evaluations", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    for digest in [Digest::of(report), Digest::of(manifest)] {
        assert_eq!(database.artifact(&digest).unwrap().unwrap().pins, 0);
    }
}
