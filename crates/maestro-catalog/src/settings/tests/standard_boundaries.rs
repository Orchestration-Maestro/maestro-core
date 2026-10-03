//! Restriction equality keeps provenance and does not report widening.
use crate::settings::{
    Layer,
    standards::{additive, bounded, chosen, constrained},
};
use maestro_settings::{Registry, SettingClass, Value};

#[test]
fn numeric_equal_requests_preserve_the_ceiling_source() {
    let registry = Registry::built_in().unwrap();
    for (key, value) in [
        ("ask.output_tokens", Value::Integer(10)),
        ("search.source_prior.weight", Value::Number(0.5)),
    ] {
        let descriptor = registry.get(key).unwrap();
        let candidates = vec![
            (Layer::Flag, value.clone()),
            (Layer::Workspace, value.clone()),
            (Layer::User, value.clone()),
        ];
        let mut diagnostics = Vec::new();
        let result = bounded(
            key,
            &descriptor.kind,
            value.clone(),
            candidates.clone(),
            &mut diagnostics,
        )
        .unwrap();
        assert_eq!(result.value(), &value);
        assert_eq!(result.source(), "user");
        assert_eq!(result.overridden(), &candidates[..2]);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }
}

#[test]
fn optional_numeric_budgets_distinguish_equal_wider_and_user_off() {
    let registry = Registry::built_in().unwrap();
    for (key, low, high) in [
        ("ask.output_tokens", Value::Integer(10), Value::Integer(20)),
        (
            "search.source_prior.weight",
            Value::Number(0.5),
            Value::Number(0.7),
        ),
    ] {
        let descriptor = registry.get(key).unwrap();
        let candidates = vec![
            (Layer::Flag, low.clone()),
            (Layer::Workspace, high),
            (Layer::User, Value::Off),
        ];
        let mut diagnostics = Vec::new();
        let result = bounded(
            key,
            &descriptor.kind,
            Value::Off,
            candidates,
            &mut diagnostics,
        )
        .unwrap();
        assert_eq!(result.value(), &low);
        assert_eq!(result.source(), "flag");
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert!(diagnostics[0].message.contains("workspace budget widening"));
        let mut diagnostics = Vec::new();
        let result = bounded(
            key,
            &descriptor.kind,
            Value::Off,
            vec![(Layer::Flag, low.clone()), (Layer::Workspace, low.clone())],
            &mut diagnostics,
        )
        .unwrap();
        assert_eq!(result.value(), &low);
        assert_eq!(result.source(), "flag");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }
}

#[test]
fn flag_diagnostics_only_name_non_user_widening() {
    let registry = Registry::built_in().unwrap();
    let descriptor = registry.get("extensions").unwrap();
    for (ceiling, requested, count) in [
        (true, true, 0),
        (true, false, 0),
        (false, false, 0),
        (false, true, 1),
    ] {
        let mut diagnostics = Vec::new();
        let result = bounded(
            "extensions",
            &descriptor.kind,
            Value::Flag(false),
            vec![
                (Layer::Flag, Value::Flag(requested)),
                (Layer::User, Value::Flag(ceiling)),
            ],
            &mut diagnostics,
        )
        .unwrap();
        assert_eq!(result.value(), &Value::Flag(ceiling && requested));
        assert_eq!(diagnostics.len(), count, "{diagnostics:?}");
    }
}

#[test]
fn ordered_equal_requests_are_not_widening_or_new_sources() {
    let registry = Registry::built_in().unwrap();
    let descriptor = registry.get("updates").unwrap();
    let mut diagnostics = Vec::new();
    let value = Value::Text("propose".into());
    let result = bounded(
        "updates",
        &descriptor.kind,
        value.clone(),
        vec![
            (Layer::Flag, value.clone()),
            (Layer::Workspace, value.clone()),
            (Layer::User, value.clone()),
        ],
        &mut diagnostics,
    )
    .unwrap();
    assert_eq!(result.value(), &value);
    assert_eq!(result.source(), "user");
    assert_eq!(result.overridden().len(), 2);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn chosen_removes_only_one_exact_source_and_value_pair() {
    let candidates = vec![
        (Layer::Flag, Value::Integer(5)),
        (Layer::Workspace, Value::Integer(5)),
        (Layer::User, Value::Integer(6)),
    ];
    let result = chosen(Value::Integer(5), Layer::Flag, candidates.clone());
    assert_eq!(result.overridden(), &candidates[1..]);
}

#[test]
fn additive_prose_inserts_newlines_only_between_nonempty_strings() {
    for (current, next, expected) in [
        ("", "", ""),
        ("", "next", "next"),
        ("current", "", "current"),
        ("current", "next", "current\nnext"),
    ] {
        let result = additive(
            "prose",
            Value::Text(current.into()),
            vec![(Layer::Workspace, Value::Text(next.into()))],
        )
        .unwrap();
        assert_eq!(result.value(), &Value::Text(expected.into()));
        assert_eq!(result.source(), "combined");
    }
}

#[test]
fn standards_admit_equal_numbers_and_unordered_text() {
    let registry = Registry::built_in().unwrap();
    for (key, baseline, narrower) in [
        (
            "search.source_prior.weight",
            Value::Number(0.7),
            Value::Number(0.5),
        ),
        (
            "tone",
            Value::Text("brief".into()),
            Value::Text("brief".into()),
        ),
    ] {
        let mut descriptor = registry.get(key).unwrap().clone();
        descriptor.class = SettingClass::Bounded;
        let result = constrained(
            &descriptor,
            Some(baseline.clone()),
            &baseline,
            &[],
            vec![(Layer::User, narrower.clone())],
        )
        .unwrap();
        assert_eq!(result.value(), &narrower);
        assert_eq!(result.source(), "user");
        if key == "tone" {
            assert!(
                constrained(
                    &descriptor,
                    Some(baseline.clone()),
                    &baseline,
                    &[],
                    vec![(Layer::User, Value::Text("normal".into()))]
                )
                .is_err()
            );
        }
    }
}
