//! The registry: every descriptor checked once, the built-in ones included,
//! and each defect named with its key.

use crate::{
    BUILT_IN, Registry, ReservedValue, SettingClass, SettingDescriptor, SettingKind, Value,
};
use std::borrow::Cow;

/// A valid free flag named `key`.
const fn flag(key: &'static str) -> SettingDescriptor {
    SettingDescriptor {
        key: Cow::Borrowed(key),
        kind: SettingKind::Flag,
        default: Cow::Borrowed("true"),
        description: Cow::Borrowed("A flag."),
        class: SettingClass::Free,
    }
}

/// Why a registry of `descriptors` is refused, rendered.
fn refusal(descriptors: &[SettingDescriptor]) -> String {
    Registry::new(descriptors)
        .map(|_| ())
        .unwrap_err()
        .to_string()
}

#[test]
fn built_in_registry_is_valid_and_resolves_each_default() {
    let registry = Registry::built_in().unwrap();
    assert_eq!(registry.descriptors().len(), BUILT_IN.len());
    assert_eq!(
        registry.default_of("tone"),
        Some(&Value::Text("normal".to_owned()))
    );
    assert_eq!(
        registry.default_of("language"),
        Some(&Value::Text("auto".to_owned()))
    );
    assert_eq!(
        registry.default_of("ask.min_rerank_score"),
        Some(&Value::Off)
    );
    assert_eq!(
        registry.default_of("search.intent.min_top_rerank"),
        Some(&Value::Off)
    );
    assert_eq!(registry.default_of("ask.output_tokens"), Some(&Value::Off));
    assert_eq!(registry.default_of("nothing"), None);
    assert!(registry.get("search.rerank.depth").is_some());
    assert!(registry.get("search.rerank").is_none());
    assert!(registry.is_table("search"));
    assert!(registry.is_table("search.rerank"));
    assert!(!registry.is_table("search.rerank.depth"));
    assert!(!registry.is_table("searc"));
    assert!(!registry.is_table(""));
}

#[test]
fn measured_search_defaults_are_registered() {
    let registry = Registry::built_in().unwrap();
    assert_eq!(
        registry.default_of("search.routes.limit"),
        Some(&Value::Integer(100))
    );
    assert_eq!(
        registry.default_of("search.routes.identifier_limit"),
        Some(&Value::Integer(20))
    );
    assert_eq!(
        registry.default_of("search.fusion_pool"),
        Some(&Value::Integer(120))
    );
    assert_eq!(
        registry.default_of("search.evidence_bytes"),
        Some(&Value::Integer(12_000))
    );
    assert_eq!(
        registry.default_of("ask.evidence_bytes"),
        Some(&Value::Integer(6_000))
    );
    assert_eq!(
        registry.default_of("evidence.expansion"),
        Some(&Value::Text("parent_chain".to_owned()))
    );
    assert_eq!(
        registry.default_of("evidence.parent_chain_order"),
        Some(&Value::Text("minimum_complete_first".to_owned()))
    );
}

#[test]
fn both_rerank_thresholds_accept_negative_values() {
    let registry = Registry::built_in().unwrap();
    for key in ["search.intent.min_top_rerank", "ask.min_rerank_score"] {
        assert_eq!(
            registry.get(key).unwrap().kind.parse_text("-1"),
            Ok(Value::Number(-1.0)),
            "{key}"
        );
    }
}

#[test]
fn setting_class_names_are_stable() {
    assert_eq!(SettingClass::Free.name(), "free");
    assert_eq!(SettingClass::Bounded.name(), "bounded");
    assert_eq!(SettingClass::Additive.name(), "additive");
    assert_eq!(SettingClass::Locked.name(), "locked");
}

#[test]
fn descriptors_round_trip_through_serde() {
    let json = serde_json::to_string(BUILT_IN).unwrap();
    let read: Vec<SettingDescriptor> = serde_json::from_str(&json).unwrap();
    assert_eq!(read, BUILT_IN);
    let tone = serde_json::to_value(&BUILT_IN[1]).unwrap();
    assert_eq!(
        tone,
        serde_json::json!({
            "key": "tone",
            "kind": {"type": "choice", "ordered": false, "values": ["brief", "normal", "detailed"]},
            "default": "normal",
            "description": BUILT_IN[1].description,
            "class": "free",
        })
    );
}

#[test]
fn new_refuses_keys_that_are_malformed_repeated_or_both_leaf_and_table() {
    for key in [
        "",
        "Search",
        "search.",
        ".search",
        "search..k",
        "1st",
        "a-b",
        "a.1b",
        "schema",
    ] {
        assert_eq!(
            refusal(&[flag(key)]),
            format!(
                "setting {key:?}: a key is dotted lower-case segments of letters, digits \
                 and '_', each starting with a letter, and never `schema`"
            ),
            "{key:?}"
        );
    }
    assert_eq!(
        refusal(&[flag("a.b"), flag("a.b")]),
        "setting \"a.b\": declared twice"
    );
    assert_eq!(
        refusal(&[flag("a.b"), flag("a")]),
        "setting \"a\": a key cannot also be the table of \"a.b\""
    );
    assert_eq!(
        refusal(&[flag("a"), flag("a.b")]),
        "setting \"a\": a key cannot also be the table of \"a.b\""
    );
    assert!(Registry::new(&[flag("a_1.b2"), flag("a_1.c")]).is_ok());
}

#[test]
fn new_refuses_kinds_and_defaults_that_cannot_hold_together() {
    let mut descriptor = flag("a");
    descriptor.default = Cow::Borrowed("yes");
    assert_eq!(
        refusal(&[descriptor]),
        "setting \"a\": its default \"yes\" is refused: expected true or false"
    );
    let mut descriptor = flag("a");
    descriptor.kind = SettingKind::Integer {
        min: 5,
        max: 1,
        off: false,
    };
    descriptor.default = Cow::Borrowed("3");
    assert_eq!(
        refusal(&[descriptor]),
        "setting \"a\": its range is empty or not finite"
    );
    let mut descriptor = flag("a");
    descriptor.kind = SettingKind::Number {
        min: 0.0,
        max: f64::INFINITY,
        off: false,
    };
    descriptor.default = Cow::Borrowed("1");
    assert_eq!(
        refusal(&[descriptor]),
        "setting \"a\": its range is empty or not finite"
    );
    for values in [&[][..], &["x", "x"][..], &["a,b"][..], &[""][..]] {
        let mut descriptor = flag("a");
        descriptor.kind = SettingKind::ChoiceList {
            values: values.iter().map(|value| Cow::Borrowed(*value)).collect(),
        };
        descriptor.default = Cow::Borrowed("");
        assert_eq!(
            refusal(&[descriptor]),
            "setting \"a\": its values are empty, repeated, or hold a comma",
            "{values:?}"
        );
    }
    let mut descriptor = flag("a");
    descriptor.kind = SettingKind::Choice {
        ordered: false,
        values: Cow::Borrowed(&[Cow::Borrowed("off"), Cow::Borrowed("gpu")]),
        reserved: Cow::Borrowed(&[ReservedValue {
            value: Cow::Borrowed("gpu"),
            reason: Cow::Borrowed("later"),
        }]),
    };
    descriptor.default = Cow::Borrowed("off");
    assert_eq!(
        refusal(&[descriptor.clone()]),
        "setting \"a\": its values are empty, repeated, or hold a comma"
    );
    descriptor.kind = SettingKind::Choice {
        ordered: false,
        values: Cow::Borrowed(&[Cow::Borrowed("off"), Cow::Borrowed("gpu")]),
        reserved: Cow::Borrowed(&[ReservedValue {
            value: Cow::Borrowed("cpu"),
            reason: Cow::Borrowed("later"),
        }]),
    };
    assert!(Registry::new(&[descriptor.clone()]).is_ok());
    descriptor.default = Cow::Borrowed("cpu");
    assert_eq!(
        refusal(&[descriptor]),
        "setting \"a\": its default \"cpu\" is refused: later"
    );
}

#[test]
fn registry_accepts_every_declared_override_class() {
    for class in [
        SettingClass::Free,
        SettingClass::Bounded,
        SettingClass::Additive,
        SettingClass::Locked,
    ] {
        let mut descriptor = flag("a");
        descriptor.class = class;
        assert!(Registry::new(&[descriptor]).is_ok(), "{}", class.name());
    }
}
#[test]
fn every_built_in_description_is_one_line_and_classes_match_architecture() {
    for descriptor in BUILT_IN {
        assert!(!descriptor.description.is_empty(), "{}", descriptor.key);
        assert!(!descriptor.description.contains('\n'), "{}", descriptor.key);
        let expected = match descriptor.key.as_ref() {
            "raw_prompt_logging"
            | "raw_reasoning_logging"
            | "provider_fallback"
            | "evidence_validation"
            | "result_validation"
            | "discovered_executable_hooks" => SettingClass::Locked,
            "updates"
            | "model_profile"
            | "reasoning_effort"
            | "ask.output_tokens"
            | "inference_writers"
            | "workspace_writers"
            | "delegation_depth"
            | "tool_calls"
            | "repair_attempts"
            | "routing_candidates"
            | "mcp_call_timeout"
            | "cross_project_memory"
            | "mcp_apps"
            | "extensions"
            | "schedules" => SettingClass::Bounded,
            _ => SettingClass::Free,
        };
        assert_eq!(descriptor.class, expected, "{}", descriptor.key);
    }
}

#[test]
fn catalog_settings_are_appended_without_changing_existing_entries() {
    let registry = Registry::built_in().unwrap();
    assert_eq!(BUILT_IN[1].key, "tone");
    for (key, default) in [
        ("updates", "propose"),
        ("model_profile", "balanced"),
        ("reasoning_effort", "default"),
        ("inference_writers", "1"),
        ("workspace_writers", "1"),
        ("delegation_depth", "2"),
        ("tool_calls", "40"),
        ("repair_attempts", "2"),
        ("routing_candidates", "3"),
        ("mcp_call_timeout", "30000"),
        ("cross_project_memory", "false"),
        ("mcp_apps", "false"),
        ("extensions", "false"),
        ("schedules", "false"),
        ("raw_prompt_logging", "false"),
        ("raw_reasoning_logging", "false"),
        ("provider_fallback", "none"),
        ("evidence_validation", "true"),
        ("result_validation", "true"),
        ("discovered_executable_hooks", "false"),
    ] {
        assert_eq!(registry.get(key).unwrap().default, default, "{key}");
    }
    assert!(registry.get("max_output_tokens").is_none());
    assert!(registry.get("ask.output_tokens").is_some());
}

#[test]
fn rerank_header_is_not_a_registered_setting() {
    let registry = Registry::built_in().unwrap();
    assert!(registry.get("search.rerank.header").is_none());
}
