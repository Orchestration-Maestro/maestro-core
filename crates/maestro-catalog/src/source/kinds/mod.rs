//! The built-in kinds: each a descriptor and, where a rule is truly
//! special, the hook it selects.

mod agent;
mod builtin;
mod instructions;
mod model_card;
mod package;
pub(crate) mod preset;
mod skill;
mod standard;
mod standard_check;
mod standard_exception;

pub use builtin::{builtin, builtin_hooks};
