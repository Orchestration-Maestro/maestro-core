use super::{manifest::manifest, manifest::parse, manifest::refusal, support::RERANKER};
use maestro_knowledge::search::{IntentExpansion, IntentTrigger, SearchConfiguration};
use serde_json::{Value, json};

#[test]
fn intent_manifest_defaults_are_library_values_and_preserve_serialization() {
    let value = manifest();
    let parsed = parse(&value).unwrap();
    let configuration = &parsed.rungs[0].configuration;
    let default_weight = SearchConfiguration::default().intent_weight;

    assert!((configuration.search().intent_weight - default_weight).abs() < f64::EPSILON);
    assert_eq!(
        serde_json::to_value(configuration).unwrap()["intent_weight"],
        json!(default_weight)
    );

    let mut explicit_null = value.clone();
    explicit_null["rungs"][0]["configuration"]["intent_weight"] = Value::Null;
    assert!(parse(&explicit_null).is_err());
}

#[test]
fn intent_manifest_requires_explicit_card_and_preserves_default_off() {
    let mut value = manifest();
    assert_eq!(
        parse(&value).unwrap().rungs[0]
            .configuration
            .search()
            .intent_expansion,
        IntentExpansion::Off
    );
    value["rungs"][0]["configuration"]["intent_expansion"] = json!("hyde");
    assert!(refusal(&value).contains("explicit intent card"));
    value["rungs"][0]["configuration"]["intent_card"] = json!(RERANKER);
    value["rungs"][0]["configuration"]["intent_trigger"] =
        json!({"low_confidence":{"min_top_rerank":2.0}});
    value["rungs"][0]["configuration"]["intent_weight"] = json!(0.5);
    value["rungs"][0]["configuration"]["intent_deadline_ms"] = json!(250);
    let configuration = parse(&value).unwrap().rungs[0].configuration.search();
    assert_eq!(configuration.intent_expansion, IntentExpansion::Hyde);
    assert_eq!(
        configuration.intent_trigger,
        IntentTrigger::LowConfidence {
            min_top_rerank: 2.0
        }
    );
    assert!((configuration.intent_weight - 0.5).abs() < f64::EPSILON);
    assert_eq!(configuration.intent_deadline_ms, 250);
    value["rungs"][0]["configuration"]["intent_deadline_ms"] = json!(5001);
    assert!(refusal(&value).contains("intent deadline"));

    value["rungs"][0]["configuration"]["intent_deadline_ms"] = json!(250);
    value["rungs"][0]["configuration"]["intent_rerank_additions"] = json!(120);
    assert!(parse(&value).is_ok(), "the maximum additions are accepted");
    value["rungs"][0]["configuration"]["intent_rerank_additions"] = json!(121);
    assert!(refusal(&value).contains("intent rerank additions"));
}
