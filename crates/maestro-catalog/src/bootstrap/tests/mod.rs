//! Bootstrap contract tests, grouped by adapter.
mod discovery;
mod inventory;
mod inventory_reuse;
mod lock_limits;
mod locks;
mod project;
mod selection;
mod snapshot;
mod support;

use super::{Preset, PresetPort, compose, inspect};
