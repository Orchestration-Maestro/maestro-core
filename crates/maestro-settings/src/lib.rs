//! Maestro's settings: everything configurable, in one registry of
//! descriptors, resolved from layered files and explicit flags.
//!
//! - [`SettingDescriptor`]: one setting as data (dotted key, kind, default,
//!   description, S3 override class); [`BUILT_IN`] lists them all.
//! - [`Registry`]: the descriptors, checked once.
//! - [`Layer::parse_preferences`]: the bounded shared file parser, accepting
//!   flat keys and `[overrides]` over the same descriptors.
//! - [`Layer`]: one `maestro-preferences/1` file, parsed strictly: an
//!   unknown key or a wrong type is refused with the key named.
//! - [`resolve()`]: built-in defaults, then the user file
//!   (`preferences.toml` in the configuration directory), then the project
//!   file (`.maestro/config.toml`, S3's workspace file, found by
//!   [`discover_project_file`]), then the `--set` flags, each value with the
//!   layer that set it.
//! - [`LayerSource`]: the port a session reads its layers through;
//!   [`FileLayers`] is the file adapter.
//! - [`FileEdit`]: one edit of a preferences file under its lock, never
//!   through a link below the directory its caller trusts, undone on
//!   request.
//! - [`set_in_document`] and [`unset_in_document`]: `config set` and
//!   `config unset`, which edit a file's text in place, keeping its comments
//!   and order.
//!
//! The kernel's `config.toml`, which holds the `[access]` grants, is
//! authority and never a preferences layer: nothing here reads or writes it.

mod builtin;
mod builtin_helpers;
mod descriptor;
mod discovery;
mod edit;
mod file;
mod language;
mod layer;
mod registry;
mod resolve;
mod store;
#[cfg(test)]
mod tests;
mod value;

pub use builtin::BUILT_IN;
pub use descriptor::{
    Reserved, ReservedValue, SettingClass, SettingDescriptor, SettingKind, Text, Texts,
};
pub use discovery::{
    Discovery, PROJECT_DIRECTORY, PROJECT_FILE, discover_project_file, discover_project_with,
};
pub use edit::{EditError, set_in_document, unset_in_document};
pub use file::{FileEdit, FileError, FileLayers, FilePlace, RestoreError};
pub use language::canonical_language;
pub use layer::{Layer, LayerError, MAX_FILE_BYTES, MAX_FILE_DEPTH, SCHEMA};
pub use registry::{Registry, RegistryError, SCHEMA_KEY};
pub use resolve::{
    Flag, LayerName, Layers, Resolved, ResolvedSetting, SettingsError, Source, parse_flags, resolve,
};
pub use store::{LayerSource, USER_FILE};
pub use value::{AUTO, OFF, Value, ValueError};
