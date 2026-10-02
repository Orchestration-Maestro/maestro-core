//! Synthetic checked trust shared by bootstrap contract tests.
use crate::{
    bootstrap::{self, BootstrapPreview, PresetPort},
    files::tests::support::with_trust,
};
use std::{io, path::Path};

/// Bind the legacy bootstrap contracts to explicit synthetic authority, not a bypass.
pub(super) fn preview(
    root: &Path,
    port: &dyn PresetPort,
    names: &[String],
) -> Result<BootstrapPreview, String> {
    with_trust(root, |trust| bootstrap::preview(root, port, names, trust))
}

/// Exercise the production checked writer with the fixture's approved root.
pub(super) fn apply(root: &Path, preview: &BootstrapPreview) -> io::Result<()> {
    with_trust(root, |trust| bootstrap::apply(root, preview, trust))
}
