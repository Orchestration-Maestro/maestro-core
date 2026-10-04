//! `skill`: an Agent Skills `SKILL.md`, whose Maestro data sits in the
//! specification's own `metadata` string map under `maestro.` keys, lists
//! joined by `;`. No hook: the descriptor states it all.

use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
    types::Maturity,
};

/// The skill kind.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        scopes: vec![
            Scope::Common,
            Scope::Core,
            Scope::Team,
            Scope::Language,
            Scope::Standard,
        ],
        kind: "skill".to_owned(),
        version: 3,
        directory: "skills".to_owned(),
        layout: Layout::Folder {
            file: "SKILL.md".to_owned(),
            data: Vec::new(),
        },
        format: Format::Markdown,
        metadata: MetadataPlace::Strings {
            key: "metadata".to_owned(),
            prefix: "maestro.".to_owned(),
            separator: ';',
        },
        name_field: Some("name".to_owned()),
        fields: vec![
            Field::required("name", FieldType::Text),
            Field::required("description", FieldType::Text),
            Field::optional("license", FieldType::Text),
            Field::optional("compatibility", FieldType::Text),
            Field::optional("allowed-tools", FieldType::Text),
        ],
        body: true,
        requires: vec!["skill".to_owned(), "instructions".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: None,
    }
}
