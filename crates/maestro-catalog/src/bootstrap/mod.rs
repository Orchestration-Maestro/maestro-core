//! Inspect a project and plan a preset composition without executing its files.
mod compose;
mod inspect;
mod project;
#[cfg(test)]
mod tests;

pub use compose::{DirectoryPresets, Preset, PresetPort, compose};
pub use inspect::{Inspection, inspect};
pub use project::{BootstrapPreview, apply, preview};
