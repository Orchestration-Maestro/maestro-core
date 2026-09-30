//! The strict checker of a catalog's authoring sources: a registry of
//! kinds described as data, generic discovery and bounded typed parsing,
//! then the checks across resources.

mod check;
mod descriptor;
mod graph;
mod kinds;
mod load;
mod metadata;
mod parse;
mod registry;
mod rules;
#[cfg(test)]
pub(crate) mod tests;
mod tree;
mod types;
mod walk;
mod yaml;

pub use check::check;
pub use descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace};
pub use kinds::{builtin, builtin_hooks, shipped_settings};
pub use registry::{Registration, Registry};
pub use tree::{Directory, Entry, EntryKind, SourceTree};
pub use types::{
    Catalog, Cause, Diagnostic, Float, Known, KnownRows, KnownSettings, Maturity, Metadata,
    Problems, Refusal, Resource, ResourceId, SCHEMA, Value, frozen_rows,
};
