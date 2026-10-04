//! The strict checker of a catalog's authoring sources: a registry of
//! kinds described as data, generic discovery and bounded typed parsing,
//! then the checks across resources.

mod area_walk;
mod backend_extensions;
mod backends;
pub(crate) mod bootstrap_inventory;
mod check;
pub(crate) mod closure;
pub(crate) mod defaults;
mod descriptor;
mod discovered;
mod graph;
pub mod index;
pub(crate) mod json;
mod kinds;
pub(crate) mod load;
mod metadata;
mod naming;
pub mod owners;
mod ownership;
pub(crate) mod parse;
mod placements;
mod references;
mod registry;
mod rules;
mod scan;
mod secrets;
mod selection;
mod standards;
#[cfg(test)]
pub(crate) mod tests;
pub(crate) mod tree;
pub(crate) mod types;
mod versions;
mod walk;
mod workflow;
mod yaml;

pub use backend_extensions::BackendExtensions;
pub use backends::{BACKENDS, BackendDescriptor};
pub use check::{build, check};
pub use descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope};
pub use kinds::{builtin, builtin_hooks};
pub use ownership::{
    BY_TYPE_PATH, CODEOWNERS_PATH, INDEX_PATH, Ownership, ReviewPath, ReviewRole, ReviewRule,
};
pub use registry::{Registration, RegistrationError, Registry};
pub use secrets::{KeychainReference, SecretReference};
pub use tree::{Directory, Entry, EntryKind, SourceTree};
pub use types::{
    Catalog, Cause, Diagnostic, Float, Known, KnownRows, KnownSettings, Maturity, Metadata,
    Problems, Refusal, Resource, ResourceId, SCHEMA, Value, frozen_rows,
};

pub(crate) use check::checked_snapshot;
pub(crate) use defaults::{DEFAULTS_PATH, from_resources};
pub(crate) use scan::Snapshot;
