use super::tests::{TestLayers, registry, resolve, value};
use maestro_settings::{BUILT_IN, Registry, Value};
use std::{borrow::Cow, collections::BTreeMap};

#[test]
fn synthetic_ordered_choice_uses_its_declared_value_order() {
    let mut descriptors = BUILT_IN.to_vec();
    descriptors.push(maestro_settings::SettingDescriptor {
        key: Cow::Borrowed("synthetic.volume"),
        kind: maestro_settings::SettingKind::Choice {
            ordered: true,
            values: Cow::Borrowed(&[
                Cow::Borrowed("quiet"),
                Cow::Borrowed("normal"),
                Cow::Borrowed("loud"),
            ]),
            reserved: Cow::Borrowed(&[]),
        },
        default: Cow::Borrowed("normal"),
        description: Cow::Borrowed("A synthetic ordered choice."),
        class: maestro_settings::SettingClass::Bounded,
        standard_only: false,
    });
    let registry = Registry::new(&descriptors).unwrap();
    let layers = TestLayers {
        user: BTreeMap::from([(
            "synthetic.volume".to_owned(),
            value(&registry, "synthetic.volume", "normal"),
        )]),
        workspace: BTreeMap::from([(
            "synthetic.volume".to_owned(),
            value(&registry, "synthetic.volume", "quiet"),
        )]),
        flags: BTreeMap::from([(
            "synthetic.volume".to_owned(),
            value(&registry, "synthetic.volume", "loud"),
        )]),
    };
    let resolved = resolve(&registry, &layers);
    let chosen = resolved.get("synthetic.volume").unwrap().as_ref().unwrap();
    assert_eq!(chosen.value(), &Value::Text("quiet".to_owned()));
    assert_eq!(chosen.source(), "workspace");
    assert_eq!(resolved.diagnostics().len(), 1);
}

#[test]
fn optional_budget_user_ceiling_is_not_reported_as_widening_after_flag_narrows() {
    let registry = registry();
    let layers = TestLayers {
        user: BTreeMap::from([(
            "ask.output_tokens".to_owned(),
            value(&registry, "ask.output_tokens", "100"),
        )]),
        flags: BTreeMap::from([(
            "ask.output_tokens".to_owned(),
            value(&registry, "ask.output_tokens", "50"),
        )]),
        ..TestLayers::default()
    };
    let resolved = resolve(&registry, &layers);
    let chosen = resolved.get("ask.output_tokens").unwrap().as_ref().unwrap();
    assert_eq!(chosen.value(), &Value::Integer(50));
    assert_eq!(chosen.source(), "flag");
    assert!(resolved.diagnostics().is_empty());
}

#[test]
fn user_enabled_permission_can_be_narrowed_by_workspace_and_flag() {
    let registry = registry();
    for (source, layers) in [
        (
            "workspace",
            TestLayers {
                user: BTreeMap::from([(
                    "mcp_apps".to_owned(),
                    value(&registry, "mcp_apps", "true"),
                )]),
                workspace: BTreeMap::from([(
                    "mcp_apps".to_owned(),
                    value(&registry, "mcp_apps", "false"),
                )]),
                ..TestLayers::default()
            },
        ),
        (
            "flag",
            TestLayers {
                user: BTreeMap::from([(
                    "mcp_apps".to_owned(),
                    value(&registry, "mcp_apps", "true"),
                )]),
                flags: BTreeMap::from([(
                    "mcp_apps".to_owned(),
                    value(&registry, "mcp_apps", "false"),
                )]),
                ..TestLayers::default()
            },
        ),
    ] {
        let resolved = resolve(&registry, &layers);
        let chosen = resolved.get("mcp_apps").unwrap().as_ref().unwrap();
        assert_eq!(chosen.value(), &Value::Flag(false));
        assert_eq!(chosen.source(), source);
    }
}
