//! Test-only pre-cutover fixtures, removed by C32's single schema cutover.

use super::{agent, builtin::builtin_hooks, instructions, mcp, model_card, preset, skill};
use crate::source::{
    descriptor::{KindDescriptor, Layout},
    registry::Registry,
};

/// The legacy test registry; never reachable from a production checker or CLI.
pub(crate) fn legacy() -> Result<Registry, String> {
    let mut registry = builtin_hooks();
    for mut descriptor in [
        agent::descriptor(),
        skill::descriptor(),
        instructions::descriptor(),
        mcp::descriptor(),
        preset::descriptor(),
        model_card::descriptor(),
    ] {
        restore(&mut descriptor);
        registry
            .register(descriptor)
            .map_err(|error| error.to_string())?;
    }
    Ok(registry)
}

/// Preserve the still-pinned /1 fixtures without a production fallback.
fn restore(descriptor: &mut KindDescriptor) {
    descriptor.scopes.clear();
    descriptor.version = 1;
    match descriptor.kind.as_str() {
        "agent" => {
            descriptor.layout = Layout::Files {
                suffix: ".agent.md".to_owned(),
                folders: vec!["base".to_owned(), "capabilities/*".to_owned()],
            }
        }
        "skill" => {
            descriptor.layout = Layout::Folder {
                file: "SKILL.md".to_owned(),
                data: vec![
                    "references".to_owned(),
                    "scripts".to_owned(),
                    "assets".to_owned(),
                ],
            }
        }
        "preset" => {
            descriptor
                .fields
                .retain(|field| !matches!(field.key.as_str(), "name" | "templates"));
            descriptor.name_field = None;
        }
        "model-card" => {
            descriptor.directory = "model-cards".to_owned();
            descriptor.layout = Layout::Files {
                suffix: ".toml".to_owned(),
                folders: vec![String::new()],
            };
        }
        _ => {}
    }
}
