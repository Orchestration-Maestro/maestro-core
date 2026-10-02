//! Inspect a project and plan a preset composition without executing its files.
mod compose;
mod inspect;
mod inventory;
mod project;
#[cfg(test)]
mod tests;

pub use compose::{Preset, PresetPort};
pub use inspect::{Inspection, inspect};
pub use inventory::AreaInventories;
pub use project::{BootstrapPreview, Prerequisite, apply, preview};
