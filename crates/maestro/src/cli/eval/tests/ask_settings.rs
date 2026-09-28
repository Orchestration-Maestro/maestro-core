//! A rung's `ask`: `true` asks as `ask` does by default, `false` does not
//! ask, and an object sets the passages, the evidence budget, the output
//! tokens and the prompt, each defaulting to today's.

use super::{
    super::{
        manifest::{AskSettings, Manifest},
        reports::RungReport,
    },
    reports::{BINARY, runs, to_json},
    support::{rung, suite},
};
use crate::failure::Failure;
use maestro_knowledge::answer::{AskBudget, PromptVersion};
use serde_json::{Value, json};
use std::path::Path;

/// A manifest of one rung whose `ask` is `ask`.
fn manifest(ask: &Value) -> Value {
    let mut rung = serde_json::to_value(rung("r0")).unwrap();
    rung["ask"] = ask.clone();
    json!({
        "schema": "maestro-ladder-manifest/1",
        "suite": "suite.jsonl",
        "collection": "docs",
        "warm_ups": 0,
        "output": "out",
        "rungs": [rung],
    })
}

/// The settings of the rung whose `ask` is `ask`.
fn parsed(ask: &Value) -> Result<Option<AskSettings>, Failure> {
    Manifest::parse(&manifest(ask).to_string(), Path::new("/ladder"))
        .map(|manifest| manifest.rungs[0].ask)
}

/// The reason the rung whose `ask` is `ask` is refused.
fn refusal(ask: &Value) -> String {
    match parsed(ask) {
        Err(Failure::Refused(reason)) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn true_and_false_still_parse_and_true_asks_as_ask_does_by_default() {
    let asks = parsed(&json!(true)).unwrap().unwrap();

    assert_eq!(parsed(&json!(false)).unwrap(), None);
    assert_eq!(asks, AskSettings::default());
    assert_eq!(asks.budget(), AskBudget::default());
    assert_eq!(asks.prompt, PromptVersion::V1);
    assert_eq!(parsed(&json!({})).unwrap(), Some(AskSettings::default()));
}

#[test]
fn an_object_sets_each_ask_setting_and_leaves_the_rest_at_their_defaults() {
    let settings = parsed(&json!({
        "k": 8, "max_tokens": 9000, "output_tokens": 900, "prompt": "v2",
    }))
    .unwrap()
    .unwrap();
    let only_k = parsed(&json!({"k": 12})).unwrap().unwrap();

    assert_eq!(
        settings.budget(),
        AskBudget {
            k: 8,
            max_tokens: 9000,
            output_tokens: 900,
            ..AskBudget::default()
        }
    );
    assert_eq!(settings.prompt, PromptVersion::V2);
    assert_eq!(
        only_k.budget(),
        AskBudget {
            k: 12,
            ..AskBudget::default()
        }
    );
    assert_eq!(only_k.prompt, PromptVersion::V1);
}

#[test]
fn ask_settings_outside_asks_limits_are_refused() {
    let cases = [
        json!({"k": 0}),
        json!({"k": 51}),
        json!({"max_tokens": 0}),
        json!({"max_tokens": 12_001}),
        json!({"output_tokens": 0}),
        json!({"output_tokens": 1025}),
    ];
    for ask in cases {
        assert!(
            refusal(&ask).contains("the rung `r0` has ask settings outside ask's limits"),
            "{ask}: {}",
            refusal(&ask)
        );
    }
    assert!(parsed(&json!({"k": 50, "max_tokens": 12_000, "output_tokens": 1024})).is_ok());
}

#[test]
fn an_unknown_prompt_or_setting_is_refused() {
    let v3 = refusal(&json!({"prompt": "v3"}));
    let thinking = refusal(&json!({"thinking": true}));
    let number = refusal(&json!(1));

    assert!(v3.contains("unknown variant `v3`"), "{v3}");
    assert!(thinking.contains("unknown field `thinking`"), "{thinking}");
    assert!(number.contains("not maestro-ladder-manifest/1"), "{number}");
}

#[test]
fn a_rung_writes_its_ask_as_true_false_or_its_settings() {
    let settings = AskSettings {
        k: Some(8),
        prompt: PromptVersion::V2,
        ..AskSettings::default()
    };
    let mut asks = rung("r0");
    let mut quiet = rung("r0");
    quiet.ask = None;
    let mut set = rung("r0");
    set.ask = Some(settings);

    assert_eq!(to_json(&asks)["ask"], json!(true));
    assert_eq!(to_json(&quiet)["ask"], json!(false));
    assert_eq!(
        to_json(&set)["ask"],
        json!({"k": 8, "max_tokens": null, "output_tokens": null, "prompt": "v2"})
    );
    asks.ask = parsed(&to_json(&set)["ask"]).unwrap();
    assert_eq!(asks, set);
}

#[test]
fn a_rung_report_records_its_resolved_ask_settings_and_prompt() {
    let mut runs = runs();
    let suite = suite(2, 1);
    runs[1].rung.ask = Some(AskSettings {
        output_tokens: Some(900),
        prompt: PromptVersion::V2,
        ..AskSettings::default()
    });
    runs[0].rung.ask = None;
    let set = RungReport::new(&runs[1], "docs", &suite.digest, BINARY);
    let unasked = RungReport::new(&runs[0], "docs", &suite.digest, BINARY);

    assert_eq!(
        to_json(&set)["ask_settings"],
        json!({"k": 5, "max_tokens": 6000, "output_tokens": 900, "prompt": "v2"})
    );
    assert!(set.to_markdown().contains(
        "- Ask settings: 5 passages, 6000 evidence bytes, 900 output tokens, prompt v2\n"
    ));
    assert_eq!(to_json(&unasked)["ask"], json!(false));
    assert_eq!(to_json(&unasked)["ask_settings"], Value::Null);
    assert!(!unasked.to_markdown().contains("Ask settings"));
}
