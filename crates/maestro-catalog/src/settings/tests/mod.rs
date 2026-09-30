//! Settings contracts over shared S1 descriptors and preference adapters.
mod discovery;
#[cfg(windows)]
mod discovery_windows;
mod preferences;
mod resolution;

pub(super) use resolution::{TestLayers, registry, resolve, value};
