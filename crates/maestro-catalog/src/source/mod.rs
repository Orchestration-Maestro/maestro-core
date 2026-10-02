//! The strict checker of a catalog's authoring sources: a registry of
//! kinds described as data, generic discovery and bounded typed parsing,
//! then the checks across resources.

mod area_walk;
pub(crate) mod bootstrap_inventory;
mod check;
pub(crate) mod closure;
mod descriptor;
mod discovered;
mod graph;
mod kinds;
mod load;
mod metadata;
mod naming;
mod ownership;
mod parse;
mod placements;
mod registry;
mod rules;
mod scan;
mod secrets;
mod selection;
mod standards;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) mod tree;
mod types;
mod walk;
mod yaml;

pub use check::{build, check};
pub use descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope};
pub use kinds::{builtin, builtin_hooks};
pub use ownership::{CODEOWNERS_PATH, Ownership, ReviewPath, ReviewRole, ReviewRule};
pub use registry::{Registration, RegistrationError, Registry};
pub use secrets::{KeychainReference, SecretReference};
pub use tree::{Directory, Entry, EntryKind, SourceTree};
pub use types::{
    Catalog, Cause, Diagnostic, Float, Known, KnownRows, KnownSettings, Maturity, Metadata,
    Problems, Refusal, Resource, ResourceId, SCHEMA, Value, frozen_rows,
};

pub(crate) use scan::Snapshot;
