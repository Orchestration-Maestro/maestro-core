//! Settings contracts over shared S1 descriptors and preference adapters.
mod default_secrets;
mod defaults;
mod discovery;
#[cfg(windows)]
mod discovery_windows;
mod instructions;
mod preferences;
mod resolution;
mod standards;

pub(super) use resolution::{TestLayers, registry, resolve, value};
mod discovery_boundaries;
mod standard_boundaries;
