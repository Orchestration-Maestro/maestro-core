//! Model card, evaluation, and selection record/read contract tests.

use super::support::{Scratch, card_for_role, collection, collection_scopes, identity, new_eval};
use crate::{
    artifact::Digest,
    gateway::{
        ModelCard, Role,
        card_v2::{FlagValue, Template, TokenizerDerivation},
    },
    model::{Error as ModelError, EvaluationDisposition, EvaluationMode, NewModelCard},
};
use std::fs;

#[test]
fn registrations_and_evaluations_round_trip_every_role_and_disposition() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "docs");
    let scopes = collection_scopes(&database, "docs");
    for (index, role) in [Role::Embedder, Role::Reranker, Role::Answerer]
        .into_iter()
        .enumerate()
    {
        let card = card_for_role(&database, &scratch, role);
        let record = database
            .record_model_card(
                &scopes,
                &NewModelCard {
                    collection_id: "docs",
                    card: &card,
                },
            )
            .unwrap();
        assert_eq!(record.role, role);
        let run_id = format!("role-{index}");
        let mut evaluation = new_eval(
            record.id,
            "docs",
            &run_id,
            EvaluationMode::Real,
            EvaluationDisposition::Interrupted,
            b"role report",
            b"role manifest",
        );
        evaluation.role = role;
        assert_eq!(
            database
                .record_model_evaluation(&scopes, &evaluation)
                .unwrap()
                .role,
            role
        );
    }

    let card = super::support::card(&database, &scratch);
    let registration = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "docs",
                card: &card,
            },
        )
        .unwrap();
    for (index, disposition) in [
        EvaluationDisposition::Eligible,
        EvaluationDisposition::Ineligible,
        EvaluationDisposition::Blocked,
        EvaluationDisposition::Failed,
        EvaluationDisposition::Interrupted,
    ]
    .into_iter()
    .enumerate()
    {
        let evaluation = database
            .record_model_evaluation(
                &scopes,
                &new_eval(
                    registration.id,
                    "docs",
                    &format!("disposition-{index}"),
                    EvaluationMode::Real,
                    disposition,
                    b"report",
                    b"manifest",
                ),
            )
            .unwrap();
        assert_eq!(evaluation.disposition, disposition);
    }
    let stored = database.model_evaluations(&scopes, "docs").unwrap();
    assert_eq!(stored.len(), 8);
    for role in [Role::Embedder, Role::Reranker, Role::Answerer] {
        assert!(stored.iter().any(|record| {
            record.role == role && record.disposition == EvaluationDisposition::Interrupted
        }));
    }
    assert_eq!(
        stored.last().unwrap().disposition,
        EvaluationDisposition::Interrupted
    );
}

#[test]
fn duplicate_card_registration_is_noop_and_preflight_failures_are_distinct_without_generation() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "docs");
    let scopes = collection_scopes(&database, "docs");
    let card = super::support::card(&database, &scratch);
    let digest = card.digest().clone();
    let new = NewModelCard {
        collection_id: "docs",
        card: &card,
    };
    let registered = database.record_model_card(&scopes, &new).unwrap();
    let pin_count = database.artifact(&digest).unwrap().unwrap().pins;
    let repeated = database.record_model_card(&scopes, &new).unwrap();
    assert_eq!(repeated, registered);
    assert_eq!(database.artifact(&digest).unwrap().unwrap().pins, pin_count);

    let manifest = br#"{"manifest":"preflight"}"#;
    let failed_report = br#"{"failure":"qualification"}"#;
    let failed = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registered.id,
                "docs",
                "preflight",
                EvaluationMode::Real,
                EvaluationDisposition::Failed,
                failed_report,
                manifest,
            ),
        )
        .unwrap();
    assert_eq!(failed.generation_id, None);
    assert_eq!(failed.manifest_digest, Digest::of(manifest));
    assert_eq!(failed.report_digest, Digest::of(failed_report));
    let rerun_report = br#"{"failure":"qualification rerun"}"#;
    let rerun = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registered.id,
                "docs",
                "preflight",
                EvaluationMode::Real,
                EvaluationDisposition::Failed,
                rerun_report,
                manifest,
            ),
        )
        .unwrap();
    assert_ne!(failed.id, rerun.id);
    assert_eq!(
        database.model_evaluations(&scopes, "docs").unwrap().len(),
        2
    );
    assert_eq!(
        card.digest(),
        &digest,
        "evaluation evidence does not change card identity"
    );
}

#[test]
fn nonembedded_tokenizer_artifacts_are_required_and_pinned() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "docs");
    let scopes = collection_scopes(&database, "docs");

    for (derivation, bytes, template_bytes) in [
        (
            TokenizerDerivation::ExternalArtifact,
            b"external tokenizer".as_slice(),
            b"external template".as_slice(),
        ),
        (
            TokenizerDerivation::NativeProduced,
            b"native tokenizer".as_slice(),
            b"native template".as_slice(),
        ),
    ] {
        let mut identity = identity(&database);
        identity.formats.tokenizer_derivation = derivation;
        identity.formats.tokenizer_digest =
            database.put(bytes, "application/octet-stream").unwrap();
        let template = database
            .put(template_bytes, "application/octet-stream")
            .unwrap();
        identity.formats.template = Template::Digest(template.clone());
        let tokenizer = identity.formats.tokenizer_digest.clone();
        let card = ModelCard::record_v2(&scratch.store(), &identity).unwrap();
        database
            .record_model_card(
                &scopes,
                &NewModelCard {
                    collection_id: "docs",
                    card: &card,
                },
            )
            .unwrap();
        assert_eq!(database.artifact(&tokenizer).unwrap().unwrap().pins, 1);
        assert_eq!(database.artifact(&template).unwrap().unwrap().pins, 1);
        database.collect_garbage().unwrap();
        assert_eq!(database.get(&tokenizer).unwrap(), bytes);
        assert_eq!(database.get(&template).unwrap(), template_bytes);
    }

    let mut missing_identity = identity(&database);
    missing_identity.formats.tokenizer_derivation = TokenizerDerivation::ExternalArtifact;
    missing_identity.formats.tokenizer_digest = Digest::of(b"missing tokenizer");
    let missing_card = ModelCard::record_v2(&scratch.store(), &missing_identity).unwrap();
    let error = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "docs",
                card: &missing_card,
            },
        )
        .unwrap_err();
    assert!(matches!(error, ModelError::Store(_)));
    let mut embedded_identity = identity(&database);
    embedded_identity.weights.gguf_digest = Digest::of(b"external GGUF");
    embedded_identity.formats.tokenizer_digest = embedded_identity.weights.gguf_digest.clone();
    embedded_identity.invocation.server_flags.insert(
        "--model".to_owned(),
        FlagValue::Asset {
            name: "weights".to_owned(),
            digest: embedded_identity.weights.gguf_digest.clone(),
        },
    );
    let embedded_tokenizer = embedded_identity.formats.tokenizer_digest.clone();
    let embedded = ModelCard::record_v2(&scratch.store(), &embedded_identity).unwrap();
    database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "docs",
                card: &embedded,
            },
        )
        .unwrap();
    assert!(
        database.artifact(&embedded_tokenizer).unwrap().is_none(),
        "GGUF-embedded tokenizer and weights stay external"
    );
}

#[test]
fn corrupted_card_artifact_is_refused_by_scoped_readers() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "docs");
    let scopes = collection_scopes(&database, "docs");
    let card = super::support::card(&database, &scratch);
    let digest = card.digest().clone();
    let registration = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "docs",
                card: &card,
            },
        )
        .unwrap();
    database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registration.id,
                "docs",
                "corrupted-card",
                EvaluationMode::Real,
                EvaluationDisposition::Eligible,
                b"corrupted card report",
                b"corrupted card manifest",
            ),
        )
        .unwrap();
    let hex = digest.as_str();
    let mut hex_chars = hex.chars();
    let first = hex_chars.by_ref().take(2).collect::<String>();
    let second = hex_chars.by_ref().take(2).collect::<String>();
    let artifact = scratch
        .0
        .join("artifacts/sha256")
        .join(first)
        .join(second)
        .join(hex);
    fs::write(artifact, b"corrupted card artifact").unwrap();
    assert!(matches!(
        database.model_card(&scopes, "docs", &digest),
        Err(ModelError::Store(_))
    ));
    assert!(matches!(
        database.model_cards(&scopes, "docs", card.fields().role),
        Err(ModelError::Store(_))
    ));
    assert!(matches!(
        database.model_evaluations(&scopes, "docs"),
        Err(ModelError::Store(_))
    ));
}

#[test]
fn evidence_artifacts_remain_pinned_and_card_json_is_the_v2_artifact() {
    let scratch = Scratch::new();
    let database = scratch.open();
    collection(&database, "docs");
    let scopes = collection_scopes(&database, "docs");
    let card = super::support::card(&database, &scratch);
    let card_digest = card.digest().clone();
    let qualification = card
        .identity()
        .unwrap()
        .formats
        .qualification_digest
        .clone();
    let registered = database
        .record_model_card(
            &scopes,
            &NewModelCard {
                collection_id: "docs",
                card: &card,
            },
        )
        .unwrap();
    let manifest = br#"{"frozen":true}"#;
    let report = br#"{"attempts":1}"#;
    let evaluation = database
        .record_model_evaluation(
            &scopes,
            &new_eval(
                registered.id,
                "docs",
                "run",
                EvaluationMode::Synthetic,
                EvaluationDisposition::Blocked,
                report,
                manifest,
            ),
        )
        .unwrap();
    database.collect_garbage().unwrap();
    assert!(database.get(&card_digest).is_ok());
    assert!(database.get(&qualification).is_ok());
    assert!(database.get(&evaluation.manifest_digest).is_ok());
    assert!(database.get(&evaluation.report_digest).is_ok());
    assert_eq!(database.artifact(&card_digest).unwrap().unwrap().pins, 1);
    assert!(
        database
            .model_card(&scopes, "docs", &card_digest)
            .unwrap()
            .unwrap()
            .identity()
            .is_some()
    );
}
