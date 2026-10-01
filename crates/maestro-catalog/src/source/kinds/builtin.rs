//! The registry of the built-in kinds and the fixed table of hooks their
//! descriptors, and any other, may select by name.

use super::{
    agent::{self, AgentRules},
    instructions, mcp, model_card,
    preset::{self, PresetRules},
    skill,
};
use crate::source::{registry::Registry, rules::KindRules};

/// The hooks a descriptor may select, by name. Content never supplies code:
/// a new hook is reviewed code added here.
const HOOKS: [(&str, &dyn KindRules, &[&str]); 3] = [
    ("agent-profile", &AgentRules, &[]),
    ("preset-settings", &PresetRules, &[]),
    ("model-card", &model_card::ModelCardRules, &["identity"]),
];

/// An empty registry whose descriptors may select the built-in hooks: the
/// seam through which kinds loaded as data register.
#[must_use]
pub fn builtin_hooks() -> Registry {
    Registry::with_hooks(&HOOKS)
}

/// The built-in kinds: agent, skill, instructions, MCP server, preset and model card.
///
/// # Errors
///
/// Why a built-in descriptor cannot be registered; a test asserts none.
pub fn builtin() -> Result<Registry, String> {
    let mut registry = builtin_hooks();
    for descriptor in [
        agent::descriptor(),
        skill::descriptor(),
        instructions::descriptor(),
        mcp::descriptor(),
        preset::descriptor(),
        model_card::descriptor(),
    ] {
        registry
            .register(descriptor)
            .map_err(|error| error.to_string())?;
    }
    Ok(registry)
}
