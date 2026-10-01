//! Exact evaluation-selection joins, insertion ordering, and selection history.

use super::support::{Scratch, collection, collection_scopes, identity, new_eval};
use crate::{
    gateway::{
        ModelCard, Role,
        card_v2::{Capability, TextFormat},
    },
    journal::{self, Filter},
    model::{
        Error as ModelError, EvaluationDisposition, EvaluationMode, NewModelCard, NewModelSelection,
    },
};
use rusqlite::params;

#[expect(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "one history scenario verifies exact joins across later cards and evaluations"
)]
#[test]
fn selection_joins_its_exact_evaluation_and_does_not_follow_later_candidates() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "docs");
    let scopes = collection_scopes(&database, "docs");
    assert!(
        database
            .selected_model_card(&scopes, "docs", Role::Embedder)
            .unwrap()
            .is_none()
    );
    let first_card = super::support::card(&database, &scratch);
    let first_digest = first_card.digest().clone();
    let first = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "docs",
                card: &first_card,
            },
        )
        .unwrap();
    let manifest = br#"{"candidate":"first"}"#;
    let report = br#"{"eligible":true}"#;
    let evaluation = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                first.id,
                "docs",
                "run-1",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                report,
                manifest,
            ),
        )
        .unwrap();
    let selection = database
        .record_model_selection(
            &scopes,
            &NewModelSelection {
                collection_id: "docs",
                role: first.role,
                card_id: first.id,
                evaluation_id: evaluation.id,
                selected_by: "owner",
                reason: "approved",
            },
        )
        .unwrap();

    let mut later_identity = identity(&database);
    later_identity.formats.query = Capability::Supported(TextFormat {
        prefix: "new query profile".to_owned(),
        suffix: String::new(),
    });
    let later_card = ModelCard::record_v2(&scratch.store(), &later_identity).unwrap();
    let later = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "docs",
                card: &later_card,
            },
        )
        .unwrap();
    assert_ne!(later.digest, first_digest);
    let error = database
        .record_model_selection(
            &scopes,
            &NewModelSelection {
                collection_id: "docs",
                role: first.role,
                card_id: later.id,
                evaluation_id: evaluation.id,
                selected_by: "owner",
                reason: "wrong card",
            },
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Invalid(_)));
    let selected = database
        .selected_model_card(&scopes, "docs", first.role)
        .unwrap()
        .unwrap();
    assert_eq!(selected.card.digest(), &first_digest);
    assert_eq!(selected.selection, selection);
    assert_eq!(selected.evaluation, evaluation);

    let manifest2 = br#"{"candidate":"first rerun"}"#;
    let report2 = br#"{"eligible":true,"repeat":2}"#;
    let evaluation2 = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                first.id,
                "docs",
                "run-2",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                report2,
                manifest2,
            ),
        )
        .unwrap();
    let mut newest = database
        .record_model_selection(
            &scopes,
            &NewModelSelection {
                collection_id: "docs",
                role: first.role,
                card_id: first.id,
                evaluation_id: evaluation2.id,
                selected_by: "owner",
                reason: "reconfirmed",
            },
        )
        .unwrap();
    let equal_time = "2026-09-27T00:00:00.000Z";
    let outside = scratch.outside();
    outside
        .execute_batch("DROP TRIGGER model_selections_are_never_updated;")
        .unwrap();
    outside
        .execute(
            "UPDATE model_selections SET recorded_at=?1 WHERE id IN (?2,?3)",
            params![equal_time, selection.id.to_string(), newest.id.to_string()],
        )
        .unwrap();
    newest.recorded_at = equal_time.to_owned();
    let selected = database
        .selected_model_card(&scopes, "docs", first.role)
        .unwrap()
        .unwrap();
    assert_eq!(
        selected.selection, newest,
        "row order, not millisecond timestamps, chooses the latest selection"
    );
    assert_eq!(selected.evaluation, evaluation2);
    assert_ne!(evaluation.manifest_digest, evaluation2.manifest_digest);

    let events = database
        .events(
            &scopes,
            &Filter {
                stream: &journal::stream("docs"),
                after: 0,
                r#type: Some("maestro.model.selected.v1"),
            },
        )
        .unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].data["card_digest"], first_digest.as_str());
    assert_eq!(events[1].data["card_digest"], first_digest.as_str());
    assert_ne!(later.id, first.id);
    assert_eq!(first_card.digest(), &first_digest);
}
