//! The root a caller names, resolved once, and the names the store appends below it.
//! The caller's root is trusted input: its links, such as macOS's `/tmp` into `/private/tmp`,
//! resolve once, so a path behaves alike on every platform. What the store appends, and anything
//! it meets below the resolved root, is walked without following a link (ADR-0018).
use std::{
    fs, io,
    path::{Component, Path, PathBuf},
};

/// The caller's root with its links resolved, followed by `below`. A root that does not exist yet
/// resolves through its deepest existing ancestor, and the rest is appended unresolved, for the
/// walk to create or refuse. Parent traversal is refused in either part, and `below` holds names
/// only.
pub(super) fn resolve(root: &Path, below: &Path) -> io::Result<PathBuf> {
    if root
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(io::Error::other("snapshot path contains parent traversal"));
    }
    if !below
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(io::Error::other(
            "snapshot path below its root holds more than names",
        ));
    }
    for existing in root.ancestors() {
        // An empty ancestor is the working directory a relative root starts from.
        let named = if existing.as_os_str().is_empty() {
            Path::new(".")
        } else {
            existing
        };
        // An ancestor that does not resolve, absent or not, is left to the walk, which creates or
        // refuses it without following a link.
        if let Ok(resolved) = fs::canonicalize(named) {
            let missing = root.strip_prefix(existing).map_err(io::Error::other)?;
            return Ok(resolved.join(missing).join(below));
        }
    }
    Err(io::ErrorKind::NotFound.into())
}
