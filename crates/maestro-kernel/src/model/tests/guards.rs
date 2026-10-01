//! Model registry write preconditions, eligibility, and transaction rollback tests.

use super::support::{Scratch, card, collection, collection_scopes, identity, new_eval};
use crate::{
    artifact::Digest,
    gateway::{CardFields, Limits, ModelCard, Role, RouterEntry},
    model::{
        Error as ModelError, EvaluationDisposition, EvaluationMode, NewModelCard, NewModelSelection,
    },
};
use std::num::{NonZeroU32, NonZeroUsize};

#[expect(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "one matrix verifies every selection eligibility boundary"
)]
#[test]
fn selections_require_the_exact_real_eligible_card_and_role() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    collection(&database, "two");
    let scopes = super::support::grant(&database, "parent", "workspace/default");
    let card = card(&database, &scratch);
    let registration = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "one",
                card: &card,
            },
        )
        .unwrap();
    for (mode, disposition) in [
        (EvaluationMode::Synthetic, EvaluationDisposition::Eligible),
        (EvaluationMode::Real, EvaluationDisposition::Ineligible),
        (EvaluationMode::Real, EvaluationDisposition::Blocked),
        (EvaluationMode::Real, EvaluationDisposition::Failed),
    ] {
        let evaluation = database
            .record_model_evaluation(
                &scopes,
                &new_eval(
                    registration.id,
                    "one",
                    "run",
                    mode,
                    disposition,
                    b"report",
                    b"manifest",
                ),
            )
            .unwrap();
        let selection = NewModelSelection {
            collection_id: "one",
            role: registration.role,
            card_id: registration.id,
            evaluation_id: evaluation.id,
            selected_by: "owner",
            reason: "test",
        };
        let error = database
            .record_model_selection(&scopes, &selection)
            .unwrap_err();
        assert!(matches!(
            error,
            ModelError::Invalid(reason) if reason.contains("eligible real evaluation")
        ));
    }
    let eligible = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registration.id,
                "one",
                "eligible",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                b"report-eligible",
                b"manifest-eligible",
            ),
        )
        .unwrap();
    let error = database
        .record_model_selection(
            &scopes,
            &NewModelSelection {
                collection_id: "one",
                role: Role::Reranker,
                card_id: registration.id,
                evaluation_id: eligible.id,
                selected_by: "owner",
                reason: "wrong role",
            },
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Invalid(_)));
    let error = database
        .record_model_selection(
            &scopes,
            &NewModelSelection {
                collection_id: "two",
                role: registration.role,
                card_id: registration.id,
                evaluation_id: eligible.id,
                selected_by: "owner",
                reason: "wrong collection",
            },
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Invalid(_)));
    let error = database
        .record_model_selection(
            &scopes,
            &NewModelSelection {
                collection_id: "one",
                role: registration.role,
                card_id: registration.id,
                evaluation_id: eligible.id,
                selected_by: " ",
                reason: "blank actor",
            },
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Invalid(reason) if reason == "selected_by is blank"));
    let error = database
        .record_model_selection(
            &scopes,
            &NewModelSelection {
                collection_id: "one",
                role: registration.role,
                card_id: registration.id,
                evaluation_id: eligible.id,
                selected_by: "owner",
                reason: " ",
            },
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Invalid(reason) if reason == "reason is blank"));
    let selected = database
        .record_model_selection(
            &scopes,
            &NewModelSelection {
                collection_id: "one",
                role: registration.role,
                card_id: registration.id,
                evaluation_id: eligible.id,
                selected_by: "owner",
                reason: "approved",
            },
        )
        .unwrap();
    assert_eq!(selected.card_id, registration.id);
}

#[test]
fn legacy_v1_cards_cannot_be_registered_as_new_candidates() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    let scopes = collection_scopes(&database, "one");
    let fields = CardFields {
        role: Role::Embedder,
        router_entry: RouterEntry::parse("embed").unwrap(),
        file_digest: Digest::of(b"legacy weights"),
        template_digest: None,
        server_build: "old-build".to_owned(),
        dimensions: NonZeroU32::new(3)
            .map(|dimension| NonZeroUsize::new(dimension.get() as usize).unwrap()),
        limits: Limits {
            context_tokens: NonZeroU32::new(8192).unwrap(),
            output_tokens: None,
        },
        suite_results: Vec::new(),
    };
    let card = ModelCard::record(&scratch.store(), &fields).unwrap();
    assert!(card.identity().is_none());
    let error = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "one",
                card: &card,
            },
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Invalid(_)));
}

#[test]
fn mandatory_pin_or_journal_failure_rolls_back_card_registration() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    let scopes = collection_scopes(&database, "one");
    let mut missing_identity = identity(&database);
    let missing = Digest::of(b"not stored qualification");
    missing_identity.formats.qualification_digest = missing.clone();
    missing_identity
        .provenance
        .artifacts
        .insert("qualification".to_owned(), missing);
    let missing_card = ModelCard::record_v2(&scratch.store(), &missing_identity).unwrap();
    let error = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "one",
                card: &missing_card,
            },
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Store(_)));
    let reader = database.reader().unwrap();
    assert_eq!(
        reader
            .query_row("SELECT count(*) FROM model_cards", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        database
            .artifact(missing_card.digest())
            .unwrap()
            .unwrap()
            .pins,
        0
    );
    drop(reader);

    let journal_scratch = Scratch::new();
    let journal_db = journal_scratch.open();
    collection(&journal_db, "one");
    let journal_scopes = collection_scopes(&journal_db, "one");
    let card = card(&journal_db, &journal_scratch);
    journal_scratch
        .outside()
        .execute_batch("DROP TABLE events")
        .unwrap();
    let error = journal_db
        .record_model_card(
            &journal_scopes,
            &NewModelCard {
                collection_id: "one",
                card: &card,
            },
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Store(_)));
    let reader = journal_db.reader().unwrap();
    assert_eq!(
        reader
            .query_row("SELECT count(*) FROM model_cards", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(journal_db.artifact(card.digest()).unwrap().unwrap().pins, 0);
}

#[test]
fn evaluation_pin_failure_rolls_back_its_record_and_pins() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "one");
    let scopes = collection_scopes(&database, "one");
    let card = card(&database, &scratch);
    let registration = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "one",
                card: &card,
            },
        )
        .unwrap();
    let manifest = b"evaluation manifest";
    let report = b"evaluation report";
    let manifest_digest = Digest::of(manifest);
    let outside = scratch.outside();
    outside
        .execute_batch(&format!(
            "CREATE TRIGGER refuse_manifest_pin BEFORE UPDATE OF pins ON artifacts \
             WHEN OLD.digest = '{}' BEGIN SELECT RAISE(ABORT, 'test pin failure'); END;",
            manifest_digest.as_str()
        ))
        .unwrap();
    let error = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registration.id,
                "one",
                "pin-failure",
                EvaluationMode::Real,
                EvaluationDisposition::Failed,
                report,
                manifest,
            ),
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Store(_)));
    assert_eq!(
        outside
            .query_row("SELECT count(*) FROM model_evaluations", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        database.artifact(&manifest_digest).unwrap().unwrap().pins,
        0
    );
    assert_eq!(
        database
            .artifact(&Digest::of(report))
            .unwrap()
            .unwrap()
            .pins,
        0
    );
}

#[test]
fn selection_journal_failure_rolls_back_its_record() {
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
    let evaluation = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registration.id,
                "one",
                "eligible",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                b"report",
                b"manifest",
            ),
        )
        .unwrap();
    scratch
        .outside()
        .execute_batch("DROP TABLE events")
        .unwrap();
    let error = database
        .record_model_selection(
            &scopes,
            &NewModelSelection {
                collection_id: "one",
                role: registration.role,
                card_id: registration.id,
                evaluation_id: evaluation.id,
                selected_by: "owner",
                reason: "journal failure",
            },
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Store(_)));
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
