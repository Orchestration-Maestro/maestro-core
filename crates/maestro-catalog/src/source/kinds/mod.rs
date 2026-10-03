//! The built-in kinds: each a descriptor and, where a rule is truly
//! special, the hook it selects.

mod agent;
mod backend;
mod bootstrap_inventory;
mod builtin;
mod contract;
mod eval;
mod handoff;
mod instructions;
mod language;
mod model_card;
mod package;
mod preset;
mod prompt;
mod quality_profile;
mod skill;
mod standard;
mod standard_check;
mod standard_exception;

pub use builtin::{builtin, builtin_hooks};
