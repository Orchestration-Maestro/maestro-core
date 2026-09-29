//! `instructions`: a Copilot `.instructions.md` file and its
//! `<name>.maestro.toml` sidecar, since the format has no extension field
//! (supervisor ruling, C03). No hook.

use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace},
    types::Maturity,
};

/// The instructions kind.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "instructions".to_owned(),
        version: 1,
        directory: "instructions".to_owned(),
        layout: Layout::Files {
            suffix: ".instructions.md".to_owned(),
            folders: vec![String::new()],
        },
        format: Format::Markdown,
        metadata: MetadataPlace::Sidecar {
            suffix: ".maestro.toml".to_owned(),
        },
        name_field: None,
        fields: vec![Field::optional("applyTo", FieldType::Text)],
        body: true,
        requires: vec!["skill".to_owned(), "instructions".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: None,
    }
}
