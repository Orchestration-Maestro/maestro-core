//! Integrity checks for scoped model registry reads.

use super::support::{
    Scratch, card, collection, collection_scopes, corrupt_artifact, identity, new_eval,
};
use crate::{
    artifact::Digest,
    gateway::{
        CardFields, Limits, ModelCard, Role, RouterEntry,
        card_v2::{Capability, TextFormat},
    },
    model::{
        CardRecord, Error as ModelError, EvaluationDisposition, EvaluationMode, NewModelCard,
        NewModelSelection,
    },
    scope::ScopeSet,
    store::Database,
};
use rusqlite::params;
use std::num::{NonZeroU32, NonZeroUsize};

fn registry() -> (Scratch, Database, ScopeSet, CardRecord) {
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
    (scratch, database, scopes, registration)
}

#[test]
fn evaluation_reads_reject_a_role_that_differs_from_its_registered_card() {
    let (scratch, database, scopes, registration) = registry();
    let evaluation = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registration.id,
                "one",
                "read-role",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                b"role report",
                b"role manifest",
            ),
        )
        .unwrap();
    let outside = scratch.outside();
    outside
        .execute_batch("DROP TRIGGER model_evaluations_are_never_updated;")
        .unwrap();
    outside
        .execute(
            "UPDATE model_evaluations SET role='reranker' WHERE id=?1",
            [evaluation.id.to_string()],
        )
        .unwrap();
    let error = database.model_evaluations(&scopes, "one").unwrap_err();
    assert!(matches!(
        error,
        ModelError::Integrity(reason) if reason == "evaluation and card roles differ"
    ));
}

#[expect(
    clippy::too_many_lines,
    reason = "one corrupt selection scenario checks every eligibility join guard"
)]
#[test]
fn selected_card_reads_require_the_exact_real_eligible_evaluation_and_role() {
    for corruption in ["card", "role", "mode", "disposition"] {
        let (scratch, database, scopes, registration) = registry();
        let other = if corruption == "card" {
            let mut different = identity(&database);
            different.formats.query = Capability::Supported(TextFormat {
                prefix: "different query".to_owned(),
                suffix: String::new(),
            });
            let different = ModelCard::record_v2(&scratch.store(), &different).unwrap();
            Some(
                database
                    .record_model_card(
                        &scopes,
                        &NewModelCard {
                            collection_id: "one",
                            card: &different,
                        },
                    )
                    .unwrap(),
            )
        } else {
            None
        };
        let evaluation = database
            .record_model_evaluation(
                &scopes,
                &new_eval(
                    registration.id,
                    "one",
                    "selected-read",
                    EvaluationMode::Real,
                    EvaluationDisposition::Eligible,
                    b"selected report",
                    b"selected manifest",
                ),
            )
            .unwrap();
        let selection = database
            .record_model_selection(
                &scopes,
                &NewModelSelection {
                    collection_id: "one",
                    role: registration.role,
                    card_id: registration.id,
                    evaluation_id: evaluation.id,
                    selected_by: "owner",
                    reason: "test selection",
                },
            )
            .unwrap();
        let outside = scratch.outside();
        match corruption {
            "card" => {
                outside
                    .execute_batch("DROP TRIGGER model_evaluations_are_never_updated;")
                    .unwrap();
                outside
                    .execute(
                        "UPDATE model_evaluations SET card_id=?1 WHERE id=?2",
                        params![other.unwrap().id.to_string(), evaluation.id.to_string()],
                    )
                    .unwrap();
            }
            "role" => {
                outside
                    .execute_batch("DROP TRIGGER model_selections_are_never_updated;")
                    .unwrap();
                outside
                    .execute(
                        "UPDATE model_selections SET role='reranker' WHERE id=?1",
                        [selection.id.to_string()],
                    )
                    .unwrap();
            }
            "mode" => {
                outside
                    .execute_batch("DROP TRIGGER model_evaluations_are_never_updated;")
                    .unwrap();
                outside
                    .execute(
                        "UPDATE model_evaluations SET mode='synthetic' WHERE id=?1",
                        [evaluation.id.to_string()],
                    )
                    .unwrap();
            }
            "disposition" => {
                outside
                    .execute_batch("DROP TRIGGER model_evaluations_are_never_updated;")
                    .unwrap();
                outside
                    .execute(
                        "UPDATE model_evaluations SET disposition='blocked' WHERE id=?1",
                        [evaluation.id.to_string()],
                    )
                    .unwrap();
            }
            _ => panic!("unknown selected-card corruption {corruption}"),
        }
        let role = if corruption == "role" {
            Role::Reranker
        } else {
            registration.role
        };
        let error = database
            .selected_model_card(&scopes, "one", role)
            .unwrap_err();
        assert!(matches!(
            error,
            ModelError::Integrity(reason)
                if reason == "selection no longer matches an eligible real evaluation"
        ));
    }
}

#[test]
fn card_reads_reject_v1_artifacts_and_row_role_mismatches() {
    let (scratch, database, scopes, registration) = registry();
    let legacy = ModelCard::record(
        &scratch.store(),
        &CardFields {
            role: Role::Embedder,
            router_entry: RouterEntry::parse("embed").unwrap(),
            file_digest: Digest::of(b"legacy weights"),
            template_digest: None,
            server_build: "old-build".to_owned(),
            dimensions: NonZeroUsize::new(1024),
            limits: Limits {
                context_tokens: NonZeroU32::new(8192).unwrap(),
                output_tokens: None,
            },
            suite_results: Vec::new(),
        },
    )
    .unwrap();
    let bytes = scratch.store().get(legacy.digest()).unwrap();
    let legacy_digest = database.put(&bytes, "application/json").unwrap();
    let outside = scratch.outside();
    outside
        .execute_batch(
            "PRAGMA ignore_check_constraints=ON; \
             DROP TRIGGER model_cards_are_never_updated;",
        )
        .unwrap();
    outside
        .execute(
            "UPDATE model_cards SET digest=?1, card_json=?2 WHERE id=?3",
            params![
                legacy_digest.as_str(),
                String::from_utf8(bytes).unwrap(),
                registration.id.to_string()
            ],
        )
        .unwrap();
    let error = database
        .model_card(&scopes, "one", &legacy_digest)
        .unwrap_err();
    assert!(matches!(
        error,
        ModelError::Integrity(reason)
            if reason == "card artifact metadata does not match its registration"
    ));

    let (scratch, database, scopes, registration) = registry();
    let outside = scratch.outside();
    outside
        .execute_batch(
            "PRAGMA ignore_check_constraints=ON; \
             DROP TRIGGER model_cards_are_never_updated;",
        )
        .unwrap();
    outside
        .execute(
            "UPDATE model_cards SET role='reranker' WHERE id=?1",
            [registration.id.to_string()],
        )
        .unwrap();
    let error = database
        .model_card(&scopes, "one", &registration.digest)
        .unwrap_err();
    assert!(matches!(
        error,
        ModelError::Integrity(reason)
            if reason == "card artifact metadata does not match its registration"
    ));
}

#[test]
fn evaluation_readers_reject_damaged_manifests_and_reports() {
    for damaged in ["manifest", "report"] {
        let (scratch, database, scopes, registration) = registry();
        let evaluation = database
            .record_model_evaluation(
                &scopes,
                &new_eval(
                    registration.id,
                    "one",
                    "damaged-evidence",
                    EvaluationMode::Real,
                    EvaluationDisposition::Eligible,
                    b"damaged report",
                    b"damaged manifest",
                ),
            )
            .unwrap();
        database
            .record_model_selection(
                &scopes,
                &NewModelSelection {
                    collection_id: "one",
                    role: registration.role,
                    card_id: registration.id,
                    evaluation_id: evaluation.id,
                    selected_by: "owner",
                    reason: "test selection",
                },
            )
            .unwrap();
        let digest = if damaged == "manifest" {
            &evaluation.manifest_digest
        } else {
            &evaluation.report_digest
        };
        corrupt_artifact(&scratch, digest);
        assert!(matches!(
            database.model_evaluations(&scopes, "one"),
            Err(ModelError::Store(_))
        ));
        assert!(matches!(
            database.selected_model_card(&scopes, "one", registration.role),
            Err(ModelError::Store(_))
        ));
    }
}
