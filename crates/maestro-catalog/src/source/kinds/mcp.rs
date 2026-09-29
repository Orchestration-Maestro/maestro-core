//! `mcp`: an approved MCP server, its launch and its tool allowlist. No
//! hook: agents' tool references are the agent kind's rule.

use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace},
    types::Maturity,
};

/// The MCP server kind.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "mcp".to_owned(),
        version: 1,
        directory: "mcp".to_owned(),
        layout: Layout::Files {
            suffix: ".toml".to_owned(),
            folders: vec![String::new()],
        },
        format: Format::Toml,
        metadata: MetadataPlace::Table {
            key: "metadata".to_owned(),
        },
        name_field: None,
        fields: vec![
            Field::required("command", FieldType::Text),
            Field::optional("args", FieldType::TextSequence),
            Field::required("tools", FieldType::ToolList),
        ],
        body: false,
        requires: Vec::new(),
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: None,
    }
}
