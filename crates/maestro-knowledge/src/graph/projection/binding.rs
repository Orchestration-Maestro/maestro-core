//! Durable input identity comparison shared by producer, reader and health.
#[cfg(any(feature = "engine", test))]
use super::{
    port::{InputMismatchKind, ProjectionError},
    settings::EngineSettings,
};
use maestro_kernel::{
    artifact::Digest,
    facts::{EXACT_RESOLVER_VERSION, PROJECTION_REBUILD_REPAIR, ProjectionReceipt},
};

/// Canonical receipt stamp values in the same fixed order.
pub(super) fn receipt_pins(receipt: &ProjectionReceipt) -> [String; 4] {
    [
        receipt.resolution_id.as_str().to_owned(),
        receipt.resolver_version.clone(),
        receipt.settings_identity.as_str().to_owned(),
        receipt.frozen_lock.as_str().to_owned(),
    ]
}

/// Strictly admit the supported resolver and three canonical digests.
pub(super) fn validate(pins: &[String; 4]) -> Result<(), String> {
    if pins[1] != EXACT_RESOLVER_VERSION
        || Digest::parse(&pins[0]).is_err()
        || Digest::parse(&pins[2]).is_err()
        || Digest::parse(&pins[3]).is_err()
    {
        return Err(format!(
            "invalid projection input pins; {PROJECTION_REBUILD_REPAIR}"
        ));
    }
    Ok(())
}

/// Compare current typed admission, never a live registry entry or a latest snapshot.
#[cfg(any(feature = "engine", test))]
pub(super) fn admitted(
    pins: &[String; 4],
    settings: &EngineSettings,
) -> Result<(), ProjectionError> {
    validate(pins).map_err(ProjectionError::Backend)?;
    if pins[2] != settings.identity().as_str() {
        return Err(ProjectionError::InputMismatch(InputMismatchKind::Settings));
    }
    if pins[3] != settings.frozen_lock().as_str() {
        return Err(ProjectionError::InputMismatch(InputMismatchKind::Lock));
    }
    Ok(())
}
