//! Read a project inventory as inert data; no script or build tool is launched.
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// Files observed at the inspected project root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Inspection {
    /// The root inspected.
    pub root: PathBuf,
    /// Root-relative immediate entries, sorted by name.
    pub entries: Vec<String>,
}

/// Inspect immediate project entries without opening or executing their contents.
///
/// # Errors
/// Returns filesystem errors encountered listing the project root.
pub fn inspect(root: &Path) -> io::Result<Inspection> {
    let mut entries = fs::read_dir(root)?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort();
    Ok(Inspection {
        root: root.to_path_buf(),
        entries,
    })
}
