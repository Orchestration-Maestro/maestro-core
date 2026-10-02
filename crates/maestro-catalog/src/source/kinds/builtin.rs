//! The registry of the built-in kinds and the fixed table of hooks their
//! descriptors, and any other, may select by name.

use super::{
    agent::{self, AgentRules},
    instructions, model_card, package,
    preset::{self, PresetRules},
    skill, standard, standard_check, standard_exception,
};
use crate::source::standards::{ExceptionRules, SettingsRules};
use crate::source::{descriptor::Scope, registry::Registry, rules::KindRules};

/// The hooks a descriptor may select, by name. Content never supplies code:
/// a new hook is reviewed code added here.
const HOOKS: [(&str, &dyn KindRules, &[&str]); 8] = [
    ("area-package", &package::PackageRules, &["settings"]),
    ("standard-settings", &SettingsRules, &["settings"]),
    ("standard-exception", &ExceptionRules, &[]),
    ("agent-profile", &AgentRules, &[]),
    ("preset-settings", &PresetRules, &[]),
    ("model-card", &model_card::ModelCardRules, &["identity"]),
    (
        "standard-inventory",
        &standard::StandardRules,
        &["settings"],
    ),
    (
        "standard-check",
        &standard_check::StandardCheckRules {
            known_validator: known_hook,
        },
        &[],
    ),
];

/// Validator membership uses the same fixed hook table as descriptor registration.
fn known_hook(name: &str) -> bool {
    HOOKS.iter().any(|(registered, _, _)| *registered == name)
}

/// An empty registry whose descriptors may select the built-in hooks: the
/// seam through which kinds loaded as data register.
#[must_use]
pub fn builtin_hooks() -> Registry {
    Registry::with_hooks(&HOOKS)
}

/// The built-in kinds: area roots, agent, skill, instructions, preset and model card.
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
        package::descriptor("package", vec![Scope::Common, Scope::Core, Scope::Team]),
        package::descriptor("language", vec![Scope::Language]),
        standard::descriptor(),
        standard_check::descriptor(),
        standard_exception::descriptor(),
        preset::descriptor(),
        model_card::descriptor(),
    ] {
        registry
            .register(descriptor)
            .map_err(|error| error.to_string())?;
    }
    Ok(registry)
}
