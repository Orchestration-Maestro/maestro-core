//! `settings`: `settings/classes.toml`, the one override class of every
//! setting Maestro knows (architecture 03 §1.6). Its hook refuses unknown
//! classes and keys, and a key with no class or two. The known settings
//! reach it through the [`KnownSettings`] port,
//! backed by S1's canonical settings registry.

use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace},
    parse::once,
    rules::KindRules,
    types::{Known, KnownSettings, Maturity, Problems, Resource, Value},
};
use std::collections::BTreeMap;

/// The override classes (architecture 03 §1.6).
const CLASSES: [&str; 4] = ["free", "bounded", "additive", "locked"];

impl KnownSettings for maestro_settings::Registry {
    fn keys(&self) -> Vec<&str> {
        self.descriptors()
            .map(|descriptor| descriptor.key.as_ref())
            .collect()
    }
}

/// The settings kind.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "settings".to_owned(),
        version: 1,
        directory: "settings".to_owned(),
        layout: Layout::Single {
            file: "classes.toml".to_owned(),
            name: "classes".to_owned(),
        },
        format: Format::Toml,
        metadata: MetadataPlace::Table {
            key: "metadata".to_owned(),
        },
        name_field: None,
        fields: vec![Field::required("classes", FieldType::ListTable)],
        body: false,
        requires: Vec::new(),
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: Some("every setting needs exactly one class".to_owned()),
        hook: Some("settings-classes".to_owned()),
    }
}

/// Gives each of `keys` the class `class`, noting keys `known` lacks and
/// keys already in another class.
fn classify<'a>(
    (class, key): (&'a str, &str),
    keys: &[&'a str],
    known: &[&str],
    class_of: &mut BTreeMap<&'a str, &'a str>,
    problems: &mut Problems,
) {
    for setting in keys {
        if !known.contains(setting) {
            problems.push((key.to_owned(), format!("unknown setting {setting:?}")));
        } else if class_of
            .insert(setting, class)
            .is_some_and(|other| other != class)
        {
            problems.push((
                "classes".to_owned(),
                format!("setting {setting:?} has two classes"),
            ));
        }
    }
}

/// The settings classes' rules beyond their descriptor.
#[derive(Debug)]
pub(super) struct SettingsRules;

impl KindRules for SettingsRules {
    fn check_resource(
        &self,
        resource: &Resource,
        _body: Option<&str>,
        known: Known<'_>,
        problems: &mut Problems,
    ) {
        let Some(Value::Table(classes)) = resource.fields.get("classes") else {
            return;
        };
        let known = known.settings.keys();
        let mut class_of: BTreeMap<&str, &str> = BTreeMap::new();
        for (class, keys) in classes {
            let key = format!("classes.{class}");
            if !CLASSES.contains(&class.as_str()) {
                problems.push((key, "unknown class".to_owned()));
                continue;
            }
            let keys = keys.texts().unwrap_or_default();
            once(keys.iter().copied(), &key, problems);
            classify((class, &key), &keys, &known, &mut class_of, problems);
        }
        for setting in known
            .iter()
            .filter(|setting| !class_of.contains_key(**setting))
        {
            problems.push((
                "classes".to_owned(),
                format!("setting {setting:?} has no class"),
            ));
        }
    }
}
