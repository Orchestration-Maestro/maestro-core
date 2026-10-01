//! `preset`: a project preset, the root of a declared closure, and the
//! settings it seeds. Its hook refuses a setting Maestro does not know.

use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
    rules::KindRules,
    types::{Known, Maturity, Problems, Resource, Value},
};

/// The preset kind.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        scopes: vec![Scope::Root],
        kind: "preset".to_owned(),
        version: 2,
        directory: "presets".to_owned(),
        layout: Layout::Files {
            suffix: ".toml".to_owned(),
            folders: vec![String::new()],
        },
        format: Format::Toml,
        metadata: MetadataPlace::Table {
            key: "metadata".to_owned(),
        },
        name_field: Some("name".to_owned()),
        fields: vec![
            Field::required("name", FieldType::Text),
            Field::required("description", FieldType::Text),
            Field::optional("templates", FieldType::TextList),
            Field::optional("settings", FieldType::ScalarTable),
        ],
        body: false,
        requires: vec!["*".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: true,
        required: None,
        hook: Some("preset-settings".to_owned()),
    }
}

/// The preset's rules beyond its descriptor.
#[derive(Debug)]
pub(super) struct PresetRules;

impl KindRules for PresetRules {
    fn check_resource(
        &self,
        resource: &Resource,
        _body: Option<&str>,
        known: Known<'_>,
        problems: &mut Problems,
    ) {
        let Some(Value::Table(settings)) = resource.fields.get("settings") else {
            return;
        };
        let keys = known.settings.keys();
        for key in settings.keys().filter(|key| !keys.contains(&key.as_str())) {
            problems.push((format!("settings.{key}"), "unknown setting".to_owned()));
        }
    }
}
