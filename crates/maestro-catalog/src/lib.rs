//! The Maestro catalog: its shared security limits and the strict checker of
//! its authoring sources. Checking reads files as bounded data; it never runs a
//! template, script or hook.
#![forbid(unsafe_code)]
/// Digest-bound, recoverable writes and removal for catalog-owned files.
pub mod files;
pub mod limits;
mod model_cards;
/// Catalog-only setting classes and restrictive resolution over S1 descriptors.
pub mod settings;
pub mod source;
