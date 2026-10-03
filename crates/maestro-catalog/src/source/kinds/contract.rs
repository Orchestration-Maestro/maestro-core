//! Native JSON Schema contracts with an exact owner-local metadata pair.

use crate::source::{
    descriptor::{Format, KindDescriptor, Layout, MetadataPlace, Scope},
    types::Maturity,
};

/// Contract placement and metadata; the graph compiler owns JSON semantics.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "contract".to_owned(),
        version: 1,
        directory: "contracts".to_owned(),
        scopes: vec![Scope::Common, Scope::Core, Scope::Team, Scope::Language],
        layout: Layout::Files {
            suffix: ".schema.json".to_owned(),
            folders: vec![String::new()],
        },
        format: Format::Json,
        metadata: MetadataPlace::Sidecar {
            suffix: ".maestro.toml".to_owned(),
        },
        name_field: None,
        fields: vec![],
        body: false,
        requires: vec!["contract".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: None,
    }
}
