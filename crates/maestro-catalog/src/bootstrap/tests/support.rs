//! Synthetic checked trust shared by bootstrap contract tests.
use crate::{
    bootstrap::{self, AreaInventories, BootstrapPreview, PresetPort},
    files::tests::support::with_trust,
    source::{Known, builtin, frozen_rows},
};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Bind the bootstrap contracts to explicit synthetic authority, not a bypass.
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

/// The production checked directory adapter with canonical registries.
pub(super) fn checked_port(root: &Path) -> Result<AreaInventories, String> {
    let settings = maestro_settings::Registry::built_in().unwrap();
    let rows = frozen_rows();
    AreaInventories::new(
        root,
        builtin().unwrap(),
        Known {
            rows: &rows,
            settings: &settings,
            today: 0,
        },
    )
}

/// Preview including constructor refusals, without bypassing source checking.
pub(super) fn checked_preview(
    root: &Path,
    catalog: &Path,
    names: &[String],
) -> Result<BootstrapPreview, String> {
    checked_port(catalog).and_then(|port| preview(root, &port, names))
}

/// Copy the synthetic checked catalog, not unrelated fixture trees.
pub(super) fn copy_catalog(root: &Path) {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/catalog/bootstrap/owner-local");
    copy_tree(&fixture, root);
}

/// Copy exact inert test files recursively; never called by production.
fn copy_tree(source: &Path, target: &Path) {
    fs::create_dir_all(target).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let destination = target.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}
