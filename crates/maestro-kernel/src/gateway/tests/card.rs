//! Tests of model cards: strict JSON artifacts whose digest is their
//! identity.

use super::{
    super::{CardError, CardFields, Limits, ModelCard, Role, RouterEntry, SuiteResult},
    fixture::{FILE_DIGEST, REPORT_DIGEST, Scratch, TEMPLATE_DIGEST, digest, fields},
};
use crate::artifact::{self, Digest};
use serde_json::{Value, json};
use std::{
    error::Error as _,
    num::{NonZeroU32, NonZeroUsize},
};

/// The card of the module's documentation: an embedder of 1024 dimensions.
fn documented() -> Value {
    json!({
        "schema": "maestro-model-card/1",
        "role": "embedder",
        "router_entry": "embed",
        "file_digest": FILE_DIGEST,
        "template_digest": null,
        "server_build": "b6500-3f2c9a1b",
        "dimensions": 1024,
        "limits": {"context_tokens": 8192, "output_tokens": null},
        "suite_results": [{"suite": "synthetic-retrieval", "report": REPORT_DIGEST}]
    })
}

/// `text` stored as an artifact, then loaded as a card.
fn load(text: &str) -> Result<ModelCard, CardError> {
    let scratch = Scratch::new();
    let store = scratch.store();
    let digest = store.put(text.as_bytes()).unwrap();
    ModelCard::load(&store, &digest)
}

/// The reason `text` is refused as a card.
fn refusal(text: &str) -> String {
    match load(text) {
        Err(CardError::Invalid(reason)) => reason,
        other => panic!("{text} was not refused as invalid: {other:?}"),
    }
}

#[test]
fn a_card_reads_the_documented_json() {
    let text = documented().to_string();
    let card = load(&text).unwrap();
    assert_eq!(card.digest(), &Digest::of(text.as_bytes()));
    let expected = CardFields {
        role: Role::Embedder,
        router_entry: RouterEntry::parse("embed").unwrap(),
        file_digest: digest(FILE_DIGEST),
        template_digest: None,
        server_build: "b6500-3f2c9a1b".to_owned(),
        dimensions: NonZeroUsize::new(1024),
        limits: Limits {
            context_tokens: NonZeroU32::new(8192).unwrap(),
            output_tokens: None,
        },
        suite_results: vec![SuiteResult {
            suite: "synthetic-retrieval".to_owned(),
            report: digest(REPORT_DIGEST),
        }],
    };
    assert_eq!(card.fields(), &expected);
}

#[test]
fn a_recorded_card_is_stored_under_its_digest_and_loads_back() {
    let scratch = Scratch::new();
    let fields = CardFields {
        limits: Limits {
            context_tokens: NonZeroU32::new(32_768).unwrap(),
            output_tokens: NonZeroU32::new(1024),
        },
        ..fields(Role::Answerer)
    };
    let recorded = ModelCard::record(&scratch.store(), &fields).unwrap();
    assert_eq!(recorded.fields(), &fields);
    let stored = scratch.store().get(recorded.digest()).unwrap();
    let written: Value = serde_json::from_slice(&stored).unwrap();
    assert_eq!(
        written,
        json!({
            "schema": "maestro-model-card/1",
            "role": "answerer",
            "router_entry": "answer",
            "file_digest": FILE_DIGEST,
            "template_digest": TEMPLATE_DIGEST,
            "server_build": "b6500-3f2c9a1b",
            "dimensions": null,
            "limits": {"context_tokens": 32_768, "output_tokens": 1024},
            "suite_results": [{"suite": "synthetic-retrieval", "report": REPORT_DIGEST}]
        })
    );
    let loaded = ModelCard::load(&scratch.store(), recorded.digest()).unwrap();
    assert_eq!(loaded, recorded);
}

#[test]
fn a_card_with_an_unknown_key_is_refused() {
    let mut top = documented();
    top["colour"] = json!("blue");
    let mut limits = documented();
    limits["limits"]["colour"] = json!("blue");
    let mut suite = documented();
    suite["suite_results"][0]["colour"] = json!("blue");
    for card in [top, limits, suite] {
        let reason = refusal(&card.to_string());
        assert!(reason.contains("unknown field `colour`"), "{reason}");
    }
}

#[test]
fn a_card_with_a_repeated_key_is_refused() {
    let text = documented()
        .to_string()
        .replacen('{', r#"{"role":"reranker","#, 1);
    let reason = refusal(&text);
    assert!(reason.contains("duplicate field `role`"), "{reason}");
}

#[test]
fn a_card_of_another_schema_is_refused() {
    let mut card = documented();
    card["schema"] = json!("maestro-model-card/2");
    let reason = refusal(&card.to_string());
    assert!(reason.contains("maestro-model-card/2"), "{reason}");
}

#[test]
fn a_card_with_a_malformed_digest_is_refused() {
    let mut file = documented();
    file["file_digest"] = json!(FILE_DIGEST.to_uppercase());
    let mut template = documented();
    template["template_digest"] = json!("sha256");
    let mut report = documented();
    report["suite_results"][0]["report"] = json!(&REPORT_DIGEST[1..]);
    for (card, field) in [
        (file, "file_digest"),
        (template, "template_digest"),
        (report, "report"),
    ] {
        let reason = refusal(&card.to_string());
        assert!(reason.contains(field), "{reason}");
    }
}

#[test]
fn a_card_with_zero_limits_or_bad_json_is_refused() {
    let mut context = documented();
    context["limits"]["context_tokens"] = json!(0);
    let mut dimensions = documented();
    dimensions["dimensions"] = json!(0);
    for text in [
        context.to_string(),
        dimensions.to_string(),
        "{".to_owned(),
        format!("{} trailing", documented()),
    ] {
        refusal(&text);
    }
}

#[test]
fn a_router_entry_is_one_path_segment() {
    for name in ["embed", "bge-m3.Q8_0", "qwen3_06b", "load"] {
        assert_eq!(RouterEntry::parse(name).unwrap().as_str(), name);
    }
    for name in [
        "", ".", "..", "a/b", "../props", "a b", "embed?x", "%2e", "modèle",
    ] {
        let error = RouterEntry::parse(name).unwrap_err();
        assert!(
            matches!(&error, CardError::Invalid(reason) if reason.contains(&format!("{name:?}"))),
            "{name:?} was accepted or refused without its name: {error}"
        );
        let mut card = documented();
        card["router_entry"] = json!(name);
        refusal(&card.to_string());
    }
}

#[test]
fn only_an_embedder_card_records_dimensions() {
    let mut embedder = documented();
    embedder["dimensions"] = Value::Null;
    let mut reranker = documented();
    reranker["role"] = json!("reranker");
    let cases = [
        (
            embedder,
            CardFields {
                dimensions: None,
                ..fields(Role::Embedder)
            },
        ),
        (
            reranker,
            CardFields {
                dimensions: NonZeroUsize::new(3),
                ..fields(Role::Reranker)
            },
        ),
    ];
    for (card, fields) in cases {
        let reason = refusal(&card.to_string());
        assert!(reason.contains("dimensions"), "{reason}");
        let scratch = Scratch::new();
        let error = ModelCard::record(&scratch.store(), &fields).unwrap_err();
        assert!(matches!(error, CardError::Invalid(_)), "{error}");
        let written = scratch.0.join("sha256");
        assert!(!written.exists(), "an invalid card is never stored");
    }
}

#[test]
fn a_card_the_store_does_not_hold_is_a_store_error() {
    let scratch = Scratch::new();
    let missing = digest(FILE_DIGEST);
    let error = ModelCard::load(&scratch.store(), &missing).unwrap_err();
    assert!(
        matches!(&error, CardError::Store(artifact::Error::Missing(found)) if *found == missing),
        "{error}"
    );
    assert!(error.to_string().contains("model card"), "{error}");
    assert!(error.source().is_some());
}

#[test]
fn an_invalid_card_says_why_and_names_the_schema() {
    let error = CardError::Invalid("dimensions: missing".to_owned());
    let message = error.to_string();
    assert!(message.contains("maestro-model-card/1"), "{message}");
    assert!(message.contains("dimensions: missing"), "{message}");
    assert!(error.source().is_none());
}
