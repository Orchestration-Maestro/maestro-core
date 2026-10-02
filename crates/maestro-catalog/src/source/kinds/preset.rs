//! `preset`: a project preset, the root of a declared closure, and the
//! settings it seeds. Its hook refuses a setting Maestro does not know.

use crate::limits::Limits;
use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
    discovered::Unit,
    load::{Context, load},
    parse::is_name,
    registry::Registration,
    rules::KindRules,
    tree::{Entry, SourceTree},
    types::{Known, Maturity, Problems, Resource, Value, frozen_rows},
};
use std::io;

/// The preset kind.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        scopes: vec![Scope::Root],
        kind: "preset".to_owned(),
        version: 3,
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
        for template in resource
            .fields
            .get("templates")
            .and_then(Value::texts)
            .unwrap_or_default()
        {
            if let Err(message) = inventory_selector(template) {
                problems.push(("templates".to_owned(), message));
            }
        }
        let Some(Value::Table(settings)) = resource.fields.get("settings") else {
            return;
        };
        let keys = known.settings.keys();
        for key in settings.keys().filter(|key| !keys.contains(&key.as_str())) {
            problems.push((format!("settings.{key}"), "unknown setting".to_owned()));
        }
    }
}

/// A template selector is two functional names, never a source path.
pub(crate) fn inventory_selector(selector: &str) -> Result<(&str, &str), String> {
    let (area, inventory) = selector
        .split_once('/')
        .filter(|(area, inventory)| is_name(area) && is_name(inventory))
        .ok_or_else(|| format!("expected area/inventory names: {selector}"))?;
    Ok((area, inventory))
}

/// Decode captured preset bytes through the same loader and kind as catalog check.
pub(crate) fn decode_preset(name: &str, bytes: &[u8]) -> Result<Resource, String> {
    if !is_name(name) {
        return Err(format!("invalid preset name: {name}"));
    }
    let registration = Registration {
        descriptor: descriptor(),
        rules: Some(&PresetRules),
    };
    let path = format!("presets/{name}.toml");
    let unit = Unit::new(&registration.descriptor, name, path.clone());
    let tree = PresetDocument { bytes };
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().map_err(|error| error.to_string())?;
    let context = Context {
        tree: &tree,
        limits: &Limits::PRODUCTION,
        known: Known {
            rows: &rows,
            settings: &settings,
            // This single-resource preset decoder does not evaluate dated exceptions.
            today: 0,
        },
    };
    load(&unit, &registration, context)
        .map(|loaded| loaded.resource)
        .map_err(|diagnostics| {
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        })
}

/// One captured primary file; decoding cannot reopen a changed source.
struct PresetDocument<'a> {
    /// Bounded bytes captured for the authoring lock.
    bytes: &'a [u8],
}

impl SourceTree for PresetDocument<'_> {
    fn list(&self, _: &str) -> io::Result<Vec<Entry>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "preset decoding does not list sources",
        ))
    }

    fn read(&self, _: &str, _: u64) -> io::Result<Vec<u8>> {
        // This registered kind reads exactly one primary file and no sidecar.
        Ok(self.bytes.to_vec())
    }
}
