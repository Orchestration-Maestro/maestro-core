//! The built-in kinds: each a descriptor and, where a rule is truly
//! special, the hook it selects.

mod agent;
mod builtin;
mod instructions;
mod mcp;
mod preset;
mod settings;
mod skill;

pub use builtin::{builtin, builtin_hooks};
pub use settings::shipped_settings;
