//! Registered extractor selection and policy file-size boundaries.

use super::runner_tests::support::{Fixture, fixture};
use crate::{
    cli::graph::extractor::{Inputs, select},
    failure::Failure,
};
use maestro_kernel::{
    gateway::{ModelCard, Role, card_v2::CardIdentity},
    model::NewModelCard,
};
use serde_json::{Value, json};
use std::{fs, path::Path};

/// Reuses the synthetic v2 identity with artifacts recorded in this kernel.
fn register(fixture: &Fixture, role: Role) -> ModelCard {
    let template = include_str!(
        "../../../../../maestro-knowledge/tests/fixtures/synthetic/evals/model-card-v2.json"
    );
    let mut identity: Value = serde_json::from_str::<Value>(template).unwrap()["identity"].clone();
    let database = &fixture.kernel.database;
    let weights = database
        .put(b"synthetic weights", "application/octet-stream")
        .unwrap();
    let runtime = database
        .put(b"synthetic runtime", "application/octet-stream")
        .unwrap();
    let qualification = database.put(b"qualification", "application/json").unwrap();
    let template = database.put(b"{{ messages }}", "text/plain").unwrap();
    identity["role"] = json!(role);
    identity["weights"]["gguf_digest"] = json!(weights);
    identity["weights"]["gguf_bytes"] = json!(17);
    identity["formats"]["tokenizer_digest"] = json!(weights);
    identity["formats"]["qualification_digest"] = json!(qualification);
    identity["formats"]["template"] = json!({"state":"digest", "digest":template});
    identity["formats"]["document"] = json!({"state":"not_applicable"});
    identity["formats"]["query"] = json!({"state":"not_applicable"});
    identity["formats"]["embedding"] = json!({"state":"not_applicable"});
    identity["invocation"]["runtime_binary_digest"] = json!(runtime);
    identity["invocation"]["server_flags"]["--model"]["value"]["digest"] = json!(weights);
    identity["invocation"]["dimensions"] = json!({"state":"not_applicable"});
    identity["invocation"]["limits"]["output_tokens"] = json!(128);
    identity["invocation"]["reasoning"] = json!({"state":"unsupported"});
    identity["invocation"]["sampling"] = json!({"state":"configured", "value":{
        "temperature":0.1,"top_p":0.9,"top_k":40,"min_p":0.0,"typical_p":1.0,
        "repeat_penalty":1.0,"frequency_penalty":0.0,"presence_penalty":0.0,"seed":1
    }});
    identity["provenance"]["artifacts"]["native-qualification"] = json!(qualification);
    let identity: CardIdentity = serde_json::from_value(identity).unwrap();
    let card = ModelCard::record_v2(&fixture.kernel.artifacts, &identity).unwrap();
    database
        .record_model_card(
            &fixture.kernel.scopes,
            &NewModelCard {
                collection_id: "graph-test",
                card: &card,
            },
        )
        .unwrap();
    card
}

fn inputs<'a>(card: &'a ModelCard, path: &'a Path) -> Inputs<'a> {
    Inputs {
        rule_path: None,
        extractor_card: Some(card.digest().as_str()),
        window_policy_path: Some(path),
        token_budget: Some(4096),
    }
}

#[test]
fn registered_extractor_selects_all_eligible_sources_at_policy_size_boundary() {
    let fixture = fixture(&["First source.", "Second source."]);
    let card = register(&fixture, Role::Extractor);
    let path = fixture.root.join("policy.json");
    let policy = json!({"schema":"maestro-graph-window-policy/1", "max_window_bytes":64,
        "overlap_bytes":0, "max_windows":8})
    .to_string();
    for size in [policy.len(), 65_536, 65_537] {
        fs::write(
            &path,
            format!("{policy}{}", " ".repeat(size - policy.len())),
        )
        .unwrap();
        let result = select(&fixture.kernel, "graph-test", inputs(&card, &path));
        if size > 65_536 {
            let expected = "window policy exceeds 65536 bytes";
            assert!(matches!(result, Err(Failure::Refused(reason)) if reason == expected));
        } else {
            assert!(
                result.is_ok(),
                "policy with {size} bytes must select an extractor"
            );
            let (extractor, revisions) = result.unwrap();
            assert_eq!(revisions, fixture.revisions);
            assert_eq!(extractor.token_budget(), Some(4096));
            assert_eq!(
                extractor.job_inputs().unwrap()["card_digest"],
                card.digest().as_str()
            );
        }
    }
}

#[test]
fn registered_non_extractor_is_refused_before_reading_policy() {
    let fixture = fixture(&["Source text."]);
    let card = register(&fixture, Role::Answerer);
    let result = select(
        &fixture.kernel,
        "graph-test",
        inputs(&card, &fixture.root.join("absent.json")),
    );
    let expected = "model card role is not Extractor";
    assert!(matches!(result, Err(Failure::Refused(reason)) if reason == expected));
}
