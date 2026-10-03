//! The Maestro catalog: its shared security limits and the strict checker of
//! its authoring sources. Checking reads files as bounded data; it never runs a
//! template, script or hook.
#![forbid(unsafe_code)]
pub mod adapters;
/// Digest-bound, recoverable writes and removal for catalog-owned files.
pub mod bootstrap;
pub mod bundle;
mod file_input;
pub mod files;
mod frontmatter;
/// Replaceable client session preference delivery.
pub mod hosts;
mod instructions;
pub mod limits;
mod model_cards;
/// Effect-free policy checks behind a replaceable evaluator and host-facts port.
pub mod policy;
/// Catalog-only setting classes and restrictive resolution over S1 descriptors.
pub mod settings;
pub mod source;
