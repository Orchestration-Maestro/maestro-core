//! The built-in kinds: each a descriptor and, where a rule is truly
//! special, the hook it selects.

mod agent;
mod builtin;
mod instructions;
mod model_card;
mod package;
mod preset;
mod skill;

pub use builtin::{builtin, builtin_hooks};
