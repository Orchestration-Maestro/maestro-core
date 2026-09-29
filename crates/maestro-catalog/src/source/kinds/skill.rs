//! `skill`: an Agent Skills `SKILL.md`, whose Maestro data sits in the
//! specification's own `metadata` string map under `maestro.` keys, lists
//! joined by `;`. No hook: the descriptor states it all.

use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace},
    types::Maturity,
};

/// The skill kind.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "skill".to_owned(),
        version: 1,
        directory: "skills".to_owned(),
        layout: Layout::Folder {
            file: "SKILL.md".to_owned(),
            data: vec![
                "references".to_owned(),
                "scripts".to_owned(),
                "assets".to_owned(),
            ],
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
