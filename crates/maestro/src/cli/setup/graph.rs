//! The embedded graph's part of `maestro setup`: it previews, and with
//! `--yes` creates, the graph's own directory under the kernel's data
//! directory and permanent control files. It downloads nothing, opens no engine
//! and removes nothing.

use crate::{failure::Failure, settings::GraphEngine};
use maestro_canonicalization::{ControlFile, OwnedRoot};
use maestro_kernel::paths::{self, Environment};
use serde::Serialize;
use std::{
    fs::{self, Metadata},
    io::{Error, ErrorKind},
    path::Path,
};

/// The graph's directory under the kernel's data directory: graph files live
/// there, never in the kernel's database.
pub(in crate::cli) const DIRECTORY: &str = "graph";

/// Whether this build holds the embedded engine `graph.engine = "ladybug"`
/// selects: only with the `engine` feature.
pub(in crate::cli) const ENGINE_BUILT: bool = cfg!(feature = "engine");

/// The graph's part of setup, in `maestro setup`'s document.
#[derive(Debug, Serialize)]
pub(super) struct GraphSetup {
    /// The selected engine: `none` or `ladybug`.
    pub(super) engine: &'static str,
    /// The graph's directory, or `null` while the graph is disabled.
    pub(super) directory: Option<String>,
    /// Permanent guard files missing before setup; preview reports these without writes.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) missing_guards: Vec<&'static str>,
    /// What the directory lacked: `disabled`, `create_directory`,
    /// `secure_permissions`, `create_guards` or `ready`.
    pub(super) action: &'static str,
    /// Whether setup created or secured the directory.
    pub(super) changed: bool,
    /// Why setup refused the graph, omitted for accepted graph states.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) detail: Option<String>,
}

/// Previews the graph's part of setup for `engine`, and takes it when `yes`.
///
/// # Errors
///
/// [`Failure::Refused`] when `ladybug` is selected in a build without the
/// engine, or the graph's path is a link or not a directory;
/// [`Failure::Failed`] when the data directory cannot be resolved or the
/// directory cannot be created or secured.
pub(super) fn run(
    environment: &Environment,
    engine: GraphEngine,
    yes: bool,
) -> Result<GraphSetup, Failure> {
    if engine == GraphEngine::None {
        return Ok(GraphSetup {
            engine: "none",
            directory: None,
            action: "disabled",
            missing_guards: Vec::new(),
            changed: false,
            detail: None,
        });
    }
    if !ENGINE_BUILT {
        return Err(Failure::refused(
            "graph.engine = \"ladybug\" needs a maestro built with the `engine` feature; \
             build it so, or set graph.engine to none",
        ));
    }
    let data = paths::data_dir(environment).map_err(|error| Failure::failed_by(&error))?;
    prepare(&data.join(DIRECTORY), yes)
}

/// Previews what `directory` lacks, and supplies it when `yes`.
fn prepare(directory: &Path, yes: bool) -> Result<GraphSetup, Failure> {
    let mut action = match fs::symlink_metadata(directory) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(Failure::refused(format!(
                "the graph directory {} is a link: move what it points to into its place \
                 under the data directory, then run setup again",
                directory.display()
            )));
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(Failure::refused(format!(
                "the graph path {} is not a directory: move it aside, then run setup again",
                directory.display()
            )));
        }
        Ok(metadata) if is_private(&metadata) => "ready",
        Ok(_) => "secure_permissions",
        Err(error) if error.kind() == ErrorKind::NotFound => "create_directory",
        Err(error) => return Err(Failure::failed_by(&error)),
    };
    // The precheck selects a report action only; all access uses the held root.
    let root = match OwnedRoot::open(directory, yes) {
        Ok(root) => Some(root),
        Err(error) if !yes && error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(unsafe_root(directory, &error)),
    };
    let controls = [ControlFile::Access, ControlFile::Writer];
    let mut missing_guards = Vec::new();
    for control in controls {
        let name = control.file_name();
        if let Some(root) = &root {
            match root.open_control(control) {
                Ok(_) => {}
                Err(error) if error.kind() == ErrorKind::NotFound => missing_guards.push(name),
                Err(error) => return Err(unsafe_root(&directory.join(name), &error)),
            }
        } else {
            missing_guards.push(name);
        }
    }
    if action == "ready" && !missing_guards.is_empty() {
        action = "create_guards";
    }
    let changed = yes && action != "ready";
    if yes && let Some(root) = &root {
        if action == "secure_permissions" {
            root.make_private()
                .map_err(|error| unsafe_root(directory, &error))?;
        }
        for control in controls {
            let name = control.file_name();
            root.ensure_control(control)
                .map_err(|error| unsafe_root(&directory.join(name), &error))?;
        }
    }
    Ok(GraphSetup {
        engine: "ladybug",
        directory: Some(directory.display().to_string()),
        missing_guards,
        action,
        changed,
        detail: None,
    })
}

/// Name the refused filesystem entry and an explicit repair action.
fn unsafe_root(path: &Path, error: &Error) -> Failure {
    Failure::refused(format!(
        "the graph path {} is unsafe or unavailable: {error}; restore its owned directory \
         and regular private guard files, then run maestro setup --yes",
        path.display()
    ))
}

/// Whether the graph directory `metadata` describes is its owner's alone:
/// mode `0700` on Unix. On Windows it inherits the data directory's ACL.
#[cfg(unix)]
pub(in crate::cli) fn is_private(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt as _;
    metadata.permissions().mode() & 0o777 == 0o700
}

/// Whether the graph directory `metadata` describes is its owner's alone:
/// mode `0700` on Unix. On Windows it inherits the data directory's ACL.
#[cfg(not(unix))]
pub(in crate::cli) fn is_private(_metadata: &Metadata) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use maestro_test_scratch::scratch_directory;
    use std::path::PathBuf;

    /// A scratch data directory, removed when dropped.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            Self(scratch_directory().unwrap())
        }

        fn graph(&self) -> PathBuf {
            self.0.join(DIRECTORY)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn a_disabled_graph_needs_nothing_and_touches_nothing() {
        let scratch = Scratch::new();
        let mut environment = Environment::default();
        environment.xdg_data_home = Some(scratch.0.clone().into_os_string());
        let setup = run(&environment, GraphEngine::None, true).unwrap();
        assert_eq!(
            (setup.engine, setup.directory, setup.action, setup.changed),
            ("none", None, "disabled", false)
        );
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
    }

    #[test]
    fn ladybug_is_refused_exactly_when_the_build_lacks_the_engine() {
        let scratch = Scratch::new();
        let mut environment = Environment::default();
        environment.xdg_data_home = Some(scratch.0.clone().into_os_string());
        let setup = run(&environment, GraphEngine::Ladybug, false);
        assert_eq!(setup.is_err(), !ENGINE_BUILT, "{setup:?}");
        if let Err(error) = setup {
            assert!(error.to_string().contains("`engine` feature"), "{error}");
        }
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
    }

    #[test]
    fn a_preview_changes_nothing_and_yes_creates_a_private_directory_once() {
        let scratch = Scratch::new();
        let graph = scratch.graph().join("nested");
        let preview = prepare(&graph, false).unwrap();
        assert_eq!(
            (preview.action, preview.changed),
            ("create_directory", false)
        );
        assert_eq!(preview.missing_guards, [".access.guard", ".writer.guard"]);
        assert_eq!(preview.engine, "ladybug");
        assert_eq!(preview.directory, Some(graph.display().to_string()));
        assert!(!scratch.graph().exists());

        let applied = prepare(&graph, true).unwrap();
        assert_eq!(
            (applied.action, applied.changed),
            ("create_directory", true)
        );
        let metadata = fs::symlink_metadata(&graph).unwrap();
        assert!(metadata.is_dir() && is_private(&metadata));

        let again = prepare(&graph, true).unwrap();
        assert_eq!((again.action, again.changed), ("ready", false));
    }

    #[test]
    fn a_file_in_the_graph_directory_s_place_is_refused_and_kept() {
        let scratch = Scratch::new();
        fs::write(scratch.graph(), b"kept").unwrap();
        let error = prepare(&scratch.graph(), true).unwrap_err();
        assert!(error.to_string().contains("not a directory"), "{error}");
        assert_eq!(fs::read(scratch.graph()).unwrap(), b"kept");
    }

    #[test]
    fn setup_previews_missing_guards_and_resumes_without_truncating() {
        let scratch = Scratch::new();
        prepare(&scratch.graph(), true).unwrap();
        fs::write(scratch.graph().join(".access.guard"), b"permanent").unwrap();
        fs::remove_file(scratch.graph().join(".writer.guard")).unwrap();
        let preview = prepare(&scratch.graph(), false).unwrap();
        assert_eq!((preview.action, preview.changed), ("create_guards", false));
        assert_eq!(preview.missing_guards, [".writer.guard"]);
        assert!(!scratch.graph().join(".writer.guard").exists());
        let resumed = prepare(&scratch.graph(), true).unwrap();
        assert_eq!((resumed.action, resumed.changed), ("create_guards", true));
        assert_eq!(
            fs::read(scratch.graph().join(".access.guard")).unwrap(),
            b"permanent"
        );
        assert!(scratch.graph().join(".writer.guard").is_file());
        let again = prepare(&scratch.graph(), true).unwrap();
        assert_eq!((again.action, again.changed), ("ready", false));
    }

    #[cfg(unix)]
    #[test]
    fn setup_refuses_linked_guard_before_creating_missing_neighbour() {
        use std::os::unix::fs::symlink;
        let scratch = Scratch::new();
        prepare(&scratch.graph(), true).unwrap();
        fs::remove_file(scratch.graph().join(".access.guard")).unwrap();
        fs::remove_file(scratch.graph().join(".writer.guard")).unwrap();
        let sentinel = scratch.0.join("outside");
        fs::write(&sentinel, b"kept").unwrap();
        symlink(&sentinel, scratch.graph().join(".writer.guard")).unwrap();
        for yes in [false, true] {
            let error = prepare(&scratch.graph(), yes).unwrap_err().to_string();
            assert!(error.contains(".writer.guard"), "{error}");
            assert!(error.contains("maestro setup --yes"), "{error}");
            assert!(!scratch.graph().join(".access.guard").exists());
            assert_eq!(fs::read(&sentinel).unwrap(), b"kept");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_shared_directory_is_previewed_then_secured() {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = Scratch::new();
        fs::create_dir(scratch.graph()).unwrap();
        fs::set_permissions(scratch.graph(), fs::Permissions::from_mode(0o750)).unwrap();
        let mode = || fs::metadata(scratch.graph()).unwrap().permissions().mode() & 0o777;
        let preview = prepare(&scratch.graph(), false).unwrap();
        assert_eq!(
            (preview.action, preview.changed, mode()),
            ("secure_permissions", false, 0o750)
        );
        let applied = prepare(&scratch.graph(), true).unwrap();
        assert_eq!(
            (applied.action, applied.changed, mode()),
            ("secure_permissions", true, 0o700)
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_linked_graph_directory_is_refused_without_following_it() {
        use std::os::unix::fs::{PermissionsExt as _, symlink};
        let scratch = Scratch::new();
        let elsewhere = scratch.0.join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        fs::set_permissions(&elsewhere, fs::Permissions::from_mode(0o755)).unwrap();
        symlink(&elsewhere, scratch.graph()).unwrap();
        let error = prepare(&scratch.graph(), true).unwrap_err();
        assert!(error.to_string().contains("is a link"), "{error}");
        assert_eq!(
            fs::metadata(&elsewhere).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }
}
