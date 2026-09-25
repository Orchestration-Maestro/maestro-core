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
    // Absolute roots always reach `/` or a drive, which resolves; only a relative root whose
    // working directory is gone resolves nowhere.
    let (existing, resolved) = root
        .ancestors()
        .find_map(|existing| {
            // An empty ancestor is the working directory a relative root starts from.
            let named = if existing.as_os_str().is_empty() {
                Path::new(".")
            } else {
                existing
            };
            // An ancestor that does not resolve, absent or not, is left to the walk, which creates
            // or refuses it without following a link.
            fs::canonicalize(named)
                .ok()
                .map(|resolved| (existing, resolved))
        })
        .ok_or(io::ErrorKind::NotFound)?;
    let missing = root.strip_prefix(existing).map_err(io::Error::other)?;
    Ok(resolved.join(missing).join(below))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{env, process};

    /// A new empty directory under the plain temporary path, named for `test`.
    fn scratch(test: &str) -> PathBuf {
        let root = env::temp_dir().join(format!("canonical-root-{}-{test}", process::id()));
        fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn a_missing_root_resolves_through_its_deepest_existing_ancestor() {
        let root = scratch("missing");
        fs::write(root.join("plain"), "not a directory").unwrap();
        let resolved = fs::canonicalize(&root).unwrap();
        let below = Path::new("identity");
        assert_eq!(
            resolve(&root.join("absent/deeper"), below).unwrap(),
            resolved.join("absent/deeper/identity")
        );
        // A file resolves, and what follows it is left for the walk to refuse.
        assert_eq!(
            resolve(&root.join("plain/deeper"), below).unwrap(),
            resolved.join("plain/deeper/identity")
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn below_the_root_only_names_are_accepted() {
        let root = scratch("names");
        for below in ["deeper/../deeper", "/deeper", "./deeper"] {
            let error = resolve(&root, Path::new(below)).unwrap_err();
            assert_eq!(
                error.to_string(),
                "snapshot path below its root holds more than names",
                "{below}"
            );
        }
        let error = resolve(&root.join("../deeper"), Path::new("")).unwrap_err();
        assert_eq!(error.to_string(), "snapshot path contains parent traversal");
        fs::remove_dir_all(root).unwrap();
    }
}
