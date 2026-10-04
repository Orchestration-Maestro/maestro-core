//! `instructions`: a Copilot `.instructions.md` file and its
//! `<name>.maestro.toml` sidecar, since the format has no extension field
//! (supervisor ruling, C03). No hook.

use crate::source::standards;
use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
    types::Maturity,
};

/// The instructions kind.
pub(super) fn descriptor() -> KindDescriptor {
    let mut descriptor = KindDescriptor {
        scopes: vec![Scope::Core, Scope::Team, Scope::Language, Scope::Standard],
        kind: "instructions".to_owned(),
        version: 4,
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
        hook: Some("standard-settings".to_owned()),
    };
    descriptor
        .fields
        .extend(standards::fields("standard-settings"));
    descriptor
}
