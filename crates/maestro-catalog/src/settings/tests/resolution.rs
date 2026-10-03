use super::super::resolve as restrictive_resolve;
use crate::source::KnownSettings;
use maestro_settings::{BUILT_IN, Flag, Layer, Layers, Registry, Value};
use std::{borrow::Cow, collections::BTreeMap, path::PathBuf};

#[derive(Default)]
pub(in crate::settings) struct TestLayers {
    pub(in crate::settings) flags: BTreeMap<String, Value>,
    pub(in crate::settings) workspace: BTreeMap<String, Value>,
    pub(in crate::settings) user: BTreeMap<String, Value>,
}

pub(in crate::settings) fn resolve(
    registry: &Registry,
    layers: &TestLayers,
) -> super::super::ResolvedSettings {
    let flags = layers
        .flags
        .iter()
        .map(|(key, value)| Flag {
            key: key.clone(),
            value: value.clone(),
        })
        .collect::<Vec<_>>();
    let workspace = to_layer(&layers.workspace);
    let user = to_layer(&layers.user);
    let s1_layers = Layers {
        project: workspace.map(|layer| (PathBuf::from("workspace.toml"), layer)),
        user: user.map(|layer| (PathBuf::from("user.toml"), layer)),
    };
    let s1_resolved = maestro_settings::resolve(registry, &s1_layers, &flags);
    restrictive_resolve(registry, &s1_resolved)
}

fn to_layer(values: &BTreeMap<String, Value>) -> Option<Layer> {
    let layer = values.iter().fold(Layer::default(), |layer, (key, value)| {
        layer.with(key, Some(value))
    });
    (!values.is_empty()).then_some(layer)
}

pub(in crate::settings) fn registry() -> Registry {
    Registry::built_in().unwrap()
}

pub(in crate::settings) fn value(registry: &Registry, key: &str, text: &str) -> Value {
    registry.get(key).unwrap().kind.parse_text(text).unwrap()
}

#[test]
fn s1_descriptors_declare_the_architecture_override_classes() {
    let registry = registry();
    for key in [
        "updates",
        "model_profile",
        "reasoning_effort",
        "ask.output_tokens",
    ] {
        assert_eq!(registry.get(key).unwrap().class.name(), "bounded", "{key}");
    }
    for key in [
        "raw_prompt_logging",
        "raw_reasoning_logging",
        "provider_fallback",
        "evidence_validation",
        "result_validation",
        "discovered_executable_hooks",
    ] {
        assert_eq!(registry.get(key).unwrap().class.name(), "locked", "{key}");
    }
}

#[test]
fn a_synthetic_s1_class_is_catalog_visible_and_drives_resolution() {
    let mut descriptors = BUILT_IN.to_vec();
    descriptors.push(maestro_settings::SettingDescriptor {
        key: Cow::Borrowed("synthetic.permission"),
        kind: maestro_settings::SettingKind::Flag,
        default: Cow::Borrowed("false"),
        description: Cow::Borrowed("A synthetic bounded setting."),
        class: maestro_settings::SettingClass::Bounded,
        standard_only: false,
    });
    let registry = Registry::new(&descriptors).unwrap();
    assert!(KnownSettings::keys(&registry).contains(&"synthetic.permission"));
    let mut layers = TestLayers::default();
    layers
        .user
        .insert("synthetic.permission".to_owned(), Value::Flag(true));
    layers
        .workspace
        .insert("synthetic.permission".to_owned(), Value::Flag(true));
    let resolved = resolve(&registry, &layers);
    let value = resolved
        .get("synthetic.permission")
        .unwrap()
        .as_ref()
        .unwrap();
    assert_eq!(value.value(), &Value::Flag(true));
    assert_eq!(value.source(), "user");
}

#[test]
fn catalog_known_settings_are_the_canonical_s1_registry_keys() {
    let registry = registry();
    let keys = registry.keys();
    assert_eq!(keys, {
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        sorted
    });
    assert_eq!(keys.len(), BUILT_IN.len());
    assert!(keys.contains(&"ask.output_tokens"));
    assert!(!keys.contains(&"max_output_tokens"));
    assert!(keys.contains(&"language"));
}

#[test]
fn free_values_resolve_flag_workspace_user_then_default() {
    let registry = registry();
    let mut layers = TestLayers::default();
    layers
        .user
        .insert("language".to_owned(), value(&registry, "language", "fr"));
    layers
        .workspace
        .insert("language".to_owned(), value(&registry, "language", "es"));
    layers
        .flags
        .insert("language".to_owned(), value(&registry, "language", "ja"));
    let resolved = resolve(&registry, &layers);
    assert_eq!(resolved.text("language"), Some("ja"));
    assert_eq!(
        resolved.get("language").unwrap().as_ref().unwrap().source(),
        "flag"
    );
    layers.flags.clear();
    assert_eq!(resolve(&registry, &layers).text("language"), Some("es"));
    layers.workspace.clear();
    assert_eq!(resolve(&registry, &layers).text("language"), Some("fr"));
    layers.user.clear();
    assert_eq!(resolve(&registry, &layers).text("language"), Some("auto"));
}

#[test]
fn updates_are_a_ceiling_and_workspace_auto_cannot_widen_user_propose() {
    let registry = registry();
    let mut layers = TestLayers::default();
    layers
        .user
        .insert("updates".to_owned(), value(&registry, "updates", "propose"));
    layers
        .workspace
        .insert("updates".to_owned(), value(&registry, "updates", "auto"));
    let resolved = resolve(&registry, &layers);
    assert_eq!(resolved.text("updates"), Some("propose"));
    assert_eq!(resolved.diagnostics().len(), 1);
}

#[test]
fn off_dominates_update_values_and_flags_cannot_raise_the_ceiling() {
    let registry = registry();
    let mut layers = TestLayers::default();
    layers
        .user
        .insert("updates".to_owned(), value(&registry, "updates", "off"));
    layers
        .workspace
        .insert("updates".to_owned(), value(&registry, "updates", "propose"));
    layers
        .flags
        .insert("updates".to_owned(), value(&registry, "updates", "auto"));
    assert_eq!(resolve(&registry, &layers).text("updates"), Some("off"));
}

#[test]
fn optional_budget_off_cannot_widen_a_user_integer_ceiling() {
    let registry = registry();
    let mut layers = TestLayers::default();
    layers.user.insert(
        "ask.output_tokens".to_owned(),
        value(&registry, "ask.output_tokens", "100"),
    );
    layers.flags.insert(
        "ask.output_tokens".to_owned(),
        value(&registry, "ask.output_tokens", "off"),
    );
    let resolved = resolve(&registry, &layers);
    assert_eq!(resolved.integer("ask.output_tokens"), Some(100));
    assert_eq!(resolved.diagnostics().len(), 1);
    assert_eq!(
        resolved
            .get("ask.output_tokens")
            .unwrap()
            .as_ref()
            .unwrap()
            .overridden()
            .len(),
        1
    );
}

#[test]
fn workspace_off_narrows_user_updates_propose() {
    let registry = registry();
    let layers = TestLayers {
        user: BTreeMap::from([("updates".to_owned(), value(&registry, "updates", "propose"))]),
        workspace: BTreeMap::from([("updates".to_owned(), value(&registry, "updates", "off"))]),
        ..TestLayers::default()
    };
    let resolved = resolve(&registry, &layers);
    assert_eq!(resolved.text("updates"), Some("off"));
    let chosen = resolved.get("updates").unwrap().as_ref().unwrap();
    assert_eq!(chosen.source(), "workspace");
    assert!(chosen.overridden().iter().any(|(layer, value)| {
        layer.name() == "user" && value == &Value::Text("propose".to_owned())
    }));
}

#[test]
fn budget_values_take_the_minimum_across_all_layers() {
    let registry = registry();
    let mut layers = TestLayers::default();
    layers.user.insert(
        "ask.output_tokens".to_owned(),
        value(&registry, "ask.output_tokens", "100"),
    );
    layers.workspace.insert(
        "ask.output_tokens".to_owned(),
        value(&registry, "ask.output_tokens", "70"),
    );
    layers.flags.insert(
        "ask.output_tokens".to_owned(),
        value(&registry, "ask.output_tokens", "80"),
    );
    let resolved = resolve(&registry, &layers);
    assert_eq!(resolved.integer("ask.output_tokens"), Some(70));
    let chosen = resolved.get("ask.output_tokens").unwrap().as_ref().unwrap();
    assert_eq!(chosen.source(), "workspace");
    assert_eq!(chosen.overridden().len(), 2);
    assert!(resolved.diagnostics().is_empty());
}

#[test]
fn locked_values_and_unknown_flag_keys_are_rejected() {
    let registry = registry();
    let mut layers = TestLayers::default();
    layers.flags.insert(
        "raw_prompt_logging".to_owned(),
        value(&registry, "raw_prompt_logging", "true"),
    );
    let resolved = resolve(&registry, &layers);
    assert!(resolved.get("raw_prompt_logging").unwrap().is_err());
}

#[test]
fn user_may_enable_bounded_permissions_but_workspace_cannot_widen_them() {
    let registry = registry();
    let mut layers = TestLayers::default();
    layers
        .user
        .insert("mcp_apps".to_owned(), value(&registry, "mcp_apps", "true"));
    assert_eq!(
        resolve(&registry, &layers)
            .get("mcp_apps")
            .unwrap()
            .as_ref()
            .unwrap()
            .value(),
        &Value::Flag(true)
    );
    layers
        .workspace
        .insert("mcp_apps".to_owned(), value(&registry, "mcp_apps", "true"));
    layers
        .user
        .insert("mcp_apps".to_owned(), value(&registry, "mcp_apps", "false"));
    let resolved = resolve(&registry, &layers);
    assert_eq!(
        resolved.get("mcp_apps").unwrap().as_ref().unwrap().value(),
        &Value::Flag(false)
    );
    assert_eq!(resolved.diagnostics().len(), 1);
}

#[test]
fn locked_validation_checks_cannot_be_dropped() {
    let registry = registry();
    let mut layers = TestLayers::default();
    layers.workspace.insert(
        "evidence_validation".to_owned(),
        value(&registry, "evidence_validation", "false"),
    );
    assert!(
        resolve(&registry, &layers)
            .get("evidence_validation")
            .unwrap()
            .is_err()
    );
    assert_eq!(
        registry.default_of("evidence_validation"),
        Some(&value(&registry, "evidence_validation", "true"))
    );
}

#[test]
fn model_profile_and_effort_use_free_precedence_within_descriptor_values() {
    let registry = registry();
    let mut layers = TestLayers::default();
    layers.user.insert(
        "model_profile".to_owned(),
        value(&registry, "model_profile", "fast"),
    );
    layers.workspace.insert(
        "model_profile".to_owned(),
        value(&registry, "model_profile", "deep"),
    );
    layers.flags.insert(
        "model_profile".to_owned(),
        value(&registry, "model_profile", "balanced"),
    );
    let resolved = resolve(&registry, &layers);
    assert_eq!(resolved.text("model_profile"), Some("balanced"));
    assert!(resolved.diagnostics().is_empty());
}

#[test]
fn only_user_preferences_can_enable_automatic_updates() {
    let registry = registry();
    let layers = TestLayers {
        user: BTreeMap::from([("updates".to_owned(), value(&registry, "updates", "auto"))]),
        ..TestLayers::default()
    };
    assert_eq!(resolve(&registry, &layers).text("updates"), Some("auto"));
}

#[test]
fn routing_candidates_use_the_restrictive_user_limit() {
    let registry = registry();
    let layers = TestLayers {
        flags: BTreeMap::from([(
            "routing_candidates".to_owned(),
            value(&registry, "routing_candidates", "3"),
        )]),
        workspace: BTreeMap::from([(
            "routing_candidates".to_owned(),
            value(&registry, "routing_candidates", "2"),
        )]),
        user: BTreeMap::from([(
            "routing_candidates".to_owned(),
            value(&registry, "routing_candidates", "1"),
        )]),
    };
    let resolved = resolve(&registry, &layers);
    assert_eq!(resolved.integer("routing_candidates"), Some(1));
    assert_eq!(
        resolved
            .get("routing_candidates")
            .unwrap()
            .as_ref()
            .unwrap()
            .source(),
        "user"
    );
}

#[test]
fn additive_checks_accumulate_without_dropping_the_default() {
    let mut descriptors = BUILT_IN.to_vec();
    descriptors.push(maestro_settings::SettingDescriptor {
        key: Cow::Borrowed("extra_checks"),
        kind: maestro_settings::SettingKind::ChoiceList {
            values: Cow::Borrowed(&[
                Cow::Borrowed("lint"),
                Cow::Borrowed("tests"),
                Cow::Borrowed("docs"),
            ]),
        },
        default: Cow::Borrowed("lint"),
        description: Cow::Borrowed("Synthetic additive checks."),
        class: maestro_settings::SettingClass::Additive,
        standard_only: false,
    });
    let registry = Registry::new(&descriptors).unwrap();
    let mut layers = TestLayers::default();
    layers.user.insert(
        "extra_checks".to_owned(),
        value(&registry, "extra_checks", "tests"),
    );
    layers.workspace.insert(
        "extra_checks".to_owned(),
        value(&registry, "extra_checks", "docs"),
    );
    let resolved = resolve(&registry, &layers);
    assert_eq!(
        resolved
            .get("extra_checks")
            .unwrap()
            .as_ref()
            .unwrap()
            .value(),
        &Value::List(vec![
            "lint".to_owned(),
            "docs".to_owned(),
            "tests".to_owned()
        ])
    );
}

#[test]
fn capability_values_are_scoped_to_their_key() {
    let registry = registry();
    let mut layers = TestLayers::default();
    layers
        .flags
        .insert("search.k".to_owned(), value(&registry, "search.k", "4"));
    assert_eq!(resolve(&registry, &layers).integer("ask.k"), Some(5));
}

#[test]
fn s1_descriptor_order_is_preserved_and_catalog_additions_are_appended() {
    assert_eq!(BUILT_IN.get(1).unwrap().key, Cow::Borrowed("tone"));
    assert_eq!(registry().get("model_profile").unwrap().default, "balanced");
    assert_eq!(registry().get("routing_candidates").unwrap().default, "3");
}

#[test]
fn resolution_does_not_depend_on_map_insertion_order() {
    let registry = registry();
    let layers = TestLayers {
        flags: BTreeMap::from([
            ("search.k".to_owned(), value(&registry, "search.k", "4")),
            ("ask.k".to_owned(), value(&registry, "ask.k", "3")),
        ]),
        ..TestLayers::default()
    };
    assert_eq!(resolve(&registry, &layers).integer("search.k"), Some(4));
}
