//! Deterministic inert bundles, independent of the authoring schema.

mod manifest;
#[cfg(test)]
mod tests;
mod write;

pub use manifest::{Bundle, Manifest};
pub use write::compile;
