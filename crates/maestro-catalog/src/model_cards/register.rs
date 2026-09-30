//! Explicit scoped registration of an already checked declaration.

use super::declaration::Declaration;
use maestro_kernel::{
    gateway::ModelCard,
    model::{CardRecord, NewModelCard},
    scope::ScopeSet,
    store::Database,
};

/// Registers a checked, reviewed declaration through the kernel registry.
///
/// The database transaction verifies that every identity-referenced artifact is
/// already pinned locally; this adapter imports no evidence or model weights.
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "C16h is the first production caller and supplies admitted installed input"
    )
)]
pub(super) fn register(
    database: &Database,
    scopes: &ScopeSet,
    collection_id: &str,
    declaration: &Declaration,
) -> Result<CardRecord, String> {
    if declaration
        .metadata
        .get("maturity")
        .and_then(toml::Value::as_str)
        != Some("reviewed")
    {
        return Err("model-card registration requires reviewed maturity".to_owned());
    }
    let card =
        ModelCard::from_identity(&declaration.identity).map_err(|error| error.to_string())?;
    database
        .record_model_card(
            scopes,
            &NewModelCard {
                collection_id,
                card: &card,
            },
        )
        .map_err(|error| error.to_string())
}
