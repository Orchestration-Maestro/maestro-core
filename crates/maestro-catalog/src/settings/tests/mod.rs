//! Settings contracts over shared S1 descriptors and preference adapters.
mod preferences;
mod resolution;

pub(super) use resolution::{TestLayers, registry, resolve, value};
