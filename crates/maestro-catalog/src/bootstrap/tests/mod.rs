//! Bootstrap contract tests, grouped by adapter.
mod discovery;
mod inventory;
mod inventory_reuse;
mod lock_limits;
mod locks;
mod nonresource;
mod project;
mod selection;
mod session_lock;
mod snapshot;
mod support;

use super::{Preset, PresetPort, compose, inspect};

mod mutation_boundaries;
mod session_ownership;
