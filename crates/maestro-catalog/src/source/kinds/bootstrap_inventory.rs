//! Strict owner-local inventories and their exact inert payload claims.
use crate::source::{
    bootstrap_inventory::{Inventory, validate_source_path, validate_top_level_name},
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
    rules::KindRules,
    types::{Known, Maturity, Problems, Resource},
};
use std::collections::BTreeSet;

/// The registered inventory placement and delegated mapping schema.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "bootstrap-inventory".to_owned(),
        version: 1,
        directory: "bootstrap".to_owned(),
        scopes: vec![
            Scope::Common,
            Scope::Core,
            Scope::Team,
            Scope::Language,
            Scope::Standard,
        ],
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
            Field::required("bindings", FieldType::TextList),
            Field::required("tools", FieldType::TextList),
            Field::required(
                "files",
                FieldType::Delegated {
                    validator: "bootstrap-inventory".to_owned(),
                },
            ),
        ],
        body: false,
        requires: vec![],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: Some("bootstrap-inventory".to_owned()),
    }
}

/// Payload claims share the same strict schema and path checks as composition.
#[derive(Debug)]
pub(super) struct InventoryRules;

impl KindRules for InventoryRules {
    fn check_resource(
        &self,
        resource: &Resource,
        _: Option<&str>,
        _: Known<'_>,
        problems: &mut Problems,
    ) {
        let result = Inventory::decoded(resource).and_then(|inventory| {
            for tool in &inventory.tools {
                validate_top_level_name(tool)?;
            }
            Ok(())
        });
        if let Err(message) = result {
            problems.push(("files".to_owned(), message));
        }
    }

    fn assets(&self, resource: &Resource) -> Result<Vec<String>, String> {
        let inventory = Inventory::decoded(resource)?;
        let parent = resource
            .path
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent);
        let mut mappings = BTreeSet::new();
        let mut sources = BTreeSet::new();
        for file in inventory.files {
            validate_source_path(&file.source)?;
            validate_source_path(&file.output)?;
            if !mappings.insert((file.source.clone(), file.output)) {
                return Err(format!(
                    "duplicate inventory source/output: {}",
                    file.source
                ));
            }
            sources.insert(format!("{parent}/{}/files/{}", inventory.name, file.source));
        }
        Ok(sources.into_iter().collect())
    }
}
