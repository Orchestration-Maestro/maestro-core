//! Observed change keys retain exact frontier provenance before coverage.
use super::controls::storage;
use crate::failure::Failure;
use maestro_acquisition::discovery::partition::captured_keys;
use maestro_kernel::acquisition::{CaptureEnvelope, ChangeKeys, Item};

/// Bind observed signals to the exact frontier item, never another capture's bytes.
pub(super) fn observed_keys(
    envelope: &CaptureEnvelope,
    item: &Item,
) -> Result<ChangeKeys, Failure> {
    if envelope.item.to_string() != item.id.to_string()
        || envelope.source != item.source
        || envelope.authorization_context != item.request.authorization_context
        || envelope.profile != item.request.representation_profile
    {
        return Err(storage());
    }
    captured_keys(envelope, None).map_err(|_| storage())
}
