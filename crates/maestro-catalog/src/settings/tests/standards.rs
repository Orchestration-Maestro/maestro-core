//! Standard values override every C17 preference layer without duplicating resolution.

use crate::settings::{resolve, resolve::resolve_with_standards};
use maestro_settings::{Flag, Layer, Layers, Registry, Value};
use std::{collections::BTreeMap, path::PathBuf};

#[test]
fn standards_constrain_user_workspace_and_flags() {
    let registry = Registry::built_in().unwrap();
    let standards = BTreeMap::from([
        (
            "ask.output_tokens".to_owned(),
            vec![Value::Integer(100), Value::Integer(50)],
        ),
        (
            "updates".to_owned(),
            vec![
                Value::Text("propose".to_owned()),
                Value::Text("off".to_owned()),
            ],
        ),
    ]);
    let empty = maestro_settings::resolve(&registry, &Layers::default(), &[]);
    let resolved = resolve_with_standards(&registry, &empty, &standards);
    assert_eq!(resolved.integer("ask.output_tokens"), Some(50));
    assert_eq!(resolved.text("updates"), Some("off"));
    for layer in [0, 1, 2] {
        let value = Value::Integer(60);
        let mut layers = Layers::default();
        let mut flags = Vec::new();
        match layer {
            0 => {
                layers.user = Some((
                    PathBuf::from("user.toml"),
                    Layer::default().with("ask.output_tokens", Some(&value)),
                ));
            }
            1 => {
                layers.project = Some((
                    PathBuf::from("workspace.toml"),
                    Layer::default().with("ask.output_tokens", Some(&value)),
                ));
            }
            _ => flags.push(Flag {
                key: "ask.output_tokens".to_owned(),
                value,
            }),
        }
        let resolved = maestro_settings::resolve(&registry, &layers, &flags);
        assert!(
            resolve_with_standards(&registry, &resolved, &standards)
                .get("ask.output_tokens")
                .unwrap()
                .is_err()
        );
    }
    let flags = [Flag {
        key: "updates".to_owned(),
        value: Value::Text("propose".to_owned()),
    }];
    let layers = maestro_settings::resolve(&registry, &Layers::default(), &flags);
    assert!(
        resolve_with_standards(&registry, &layers, &standards)
            .get("updates")
            .unwrap()
            .is_err()
    );
}

#[test]
fn descriptor_classes_drive_standard_list_intersection_and_accumulation() {
    let mut descriptors = maestro_settings::BUILT_IN.to_vec();
    let list = descriptors
        .iter_mut()
        .find(|descriptor| descriptor.key == "search.section_prior.classes")
        .unwrap();
    list.class = maestro_settings::SettingClass::Bounded;
    let registry = Registry::new(&descriptors).unwrap();
    let standards = BTreeMap::from([(
        "search.section_prior.classes".to_owned(),
        vec![
            Value::List(vec!["changelog".to_owned(), "conversion".to_owned()]),
            Value::List(vec!["changelog".to_owned()]),
        ],
    )]);
    let layers = maestro_settings::resolve(&registry, &Layers::default(), &[]);
    let resolved = resolve_with_standards(&registry, &layers, &standards);
    assert_eq!(
        resolved
            .get("search.section_prior.classes")
            .unwrap()
            .as_ref()
            .unwrap()
            .value(),
        &Value::List(vec!["changelog".to_owned()])
    );
    descriptors
        .iter_mut()
        .find(|descriptor| descriptor.key == "search.section_prior.classes")
        .unwrap()
        .class = maestro_settings::SettingClass::Additive;
    let registry = Registry::new(&descriptors).unwrap();
    let layers = maestro_settings::resolve(&registry, &Layers::default(), &[]);
    let resolved = resolve_with_standards(&registry, &layers, &standards);
    assert_eq!(
        resolved
            .get("search.section_prior.classes")
            .unwrap()
            .as_ref()
            .unwrap()
            .value(),
        &Value::List(vec!["changelog".to_owned(), "conversion".to_owned()])
    );
}

#[test]
fn descriptor_declared_number_ceiling_uses_c17_minimum() {
    let mut descriptors = maestro_settings::BUILT_IN.to_vec();
    descriptors
        .iter_mut()
        .find(|descriptor| descriptor.key == "search.source_prior.weight")
        .unwrap()
        .class = maestro_settings::SettingClass::Bounded;
    let registry = Registry::new(&descriptors).unwrap();
    let standards = BTreeMap::from([(
        "search.source_prior.weight".to_owned(),
        vec![Value::Number(0.7), Value::Number(0.5), Value::Number(0.6)],
    )]);
    let layers = maestro_settings::resolve(&registry, &Layers::default(), &[]);
    assert_eq!(
        resolve_with_standards(&registry, &layers, &standards)
            .get("search.source_prior.weight")
            .unwrap()
            .as_ref()
            .unwrap()
            .value(),
        &Value::Number(0.5)
    );
    let layers = Layers {
        user: Some((
            PathBuf::from("user.toml"),
            Layer::default().with("search.source_prior.weight", Some(&Value::Number(0.5))),
        )),
        project: Some((
            PathBuf::from("project.toml"),
            Layer::default().with("search.source_prior.weight", Some(&Value::Number(0.6))),
        )),
    };
    let layers = maestro_settings::resolve(&registry, &layers, &[]);
    assert!(
        resolve_with_standards(&registry, &layers, &standards)
            .get("search.source_prior.weight")
            .unwrap()
            .is_err()
    );
    assert_eq!(resolve(&registry, &layers).diagnostics().len(), 1);
}

#[test]
fn list_permissions_intersect_and_explain_local_additions() {
    let mut descriptors = maestro_settings::BUILT_IN.to_vec();
    descriptors
        .iter_mut()
        .find(|descriptor| descriptor.key == "search.section_prior.classes")
        .unwrap()
        .class = maestro_settings::SettingClass::Bounded;
    let registry = Registry::new(&descriptors).unwrap();
    let layers = Layers {
        user: Some((
            PathBuf::from("user.toml"),
            Layer::default().with(
                "search.section_prior.classes",
                Some(&Value::List(vec!["changelog".to_owned()])),
            ),
        )),
        ..Layers::default()
    };
    let flags = [Flag {
        key: "search.section_prior.classes".to_owned(),
        value: Value::List(vec!["conversion".to_owned()]),
    }];
    let layers = maestro_settings::resolve(&registry, &layers, &flags);
    let resolved = resolve(&registry, &layers);
    assert_eq!(
        resolved
            .get("search.section_prior.classes")
            .unwrap()
            .as_ref()
            .unwrap()
            .value(),
        &Value::List(Vec::new())
    );
    assert_eq!(resolved.diagnostics().len(), 1);
    assert!(
        resolved.diagnostics()[0]
            .message
            .contains("permission list widening")
    );
}
