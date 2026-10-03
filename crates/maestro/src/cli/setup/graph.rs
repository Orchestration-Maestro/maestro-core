//! The embedded graph's part of `maestro setup`: it previews, and with
//! `--yes` creates, the graph's own directory under the kernel's data
//! directory and permanent control files. It downloads nothing, opens no engine
//! and removes nothing.

use crate::failure::Failure;
#[cfg(not(feature = "engine"))]
use crate::settings::GraphEngine;
use maestro_canonicalization::{ControlFile, OwnedRoot};
#[cfg(not(feature = "engine"))]
use maestro_kernel::paths::Environment;
use serde::Serialize;
use std::{
    fs::{self, Metadata},
    io::{Error, ErrorKind},
    path::Path,
};

/// The graph's directory under the kernel's data directory: graph files live
/// there, never in the kernel's database.
pub(in crate::cli) const DIRECTORY: &str = "graph";

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

/// The disabled graph's unchanged document, shared by both feature builds.
pub(super) fn disabled() -> GraphSetup {
    GraphSetup {
        engine: "none",
        directory: None,
        action: "disabled",
        missing_guards: Vec::new(),
        changed: false,
        detail: None,
    }
}

/// Report a disabled graph or refuse its selected engine in a featureless build.
///
/// # Errors
/// [`Failure::Refused`] when `ladybug` is selected without the engine feature.
#[cfg(not(feature = "engine"))]
pub(super) fn run(
    _environment: &Environment,
    engine: GraphEngine,
    _yes: bool,
) -> Result<GraphSetup, Failure> {
    if engine == GraphEngine::None {
        return Ok(disabled());
    }
    Err(Failure::refused(
        "graph.engine = \"ladybug\" needs a maestro built with the `engine` feature; \
         build it so, or set graph.engine to none",
    ))
}

/// Previews what `directory` lacks, and supplies it when `yes`.
#[cfg_attr(
    all(not(feature = "engine"), not(test)),
    expect(
        dead_code,
        reason = "default tests and the engine adapter share filesystem preparation"
    )
)]
pub(super) fn prepare(directory: &Path, yes: bool) -> Result<GraphSetup, Failure> {
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
        if action == "create_directory" || action == "secure_permissions" {
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
    metadata.permissions().mode() & 0o7777 == 0o700
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
        let setup = disabled();
        assert_eq!(
            (setup.engine, setup.directory, setup.action, setup.changed),
            ("none", None, "disabled", false)
        );
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
    }

    #[cfg(not(feature = "engine"))]
    #[test]
    fn ladybug_is_refused_without_the_engine_and_creates_nothing() {
        let scratch = Scratch::new();
        let mut environment = Environment::default();
        environment.xdg_data_home = Some(scratch.0.clone().into_os_string());
        for yes in [false, true] {
            let error = run(&environment, GraphEngine::Ladybug, yes).unwrap_err();
            assert_eq!(
                error.to_string(),
                "graph.engine = \"ladybug\" needs a maestro built \
                with the `engine` feature; build it so, or set graph.engine to none"
            );
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
    fn setup_refuses_owned_root_errors_in_preview_and_apply() {
        let scratch = Scratch::new();
        prepare(&scratch.graph(), true).unwrap();
        fs::create_dir(scratch.0.join("nested")).unwrap();
        let traversing = scratch.0.join("nested/../graph");
        for yes in [false, true] {
            let error = prepare(&traversing, yes).unwrap_err().to_string();
            assert!(error.contains("parent traversal"), "{error}");
            assert!(error.contains("maestro setup --yes"), "{error}");
        }
        assert!(scratch.graph().join(".access.guard").is_file());
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

    /// Retained control identities, modes and bytes; reads never replace a guard.
    #[cfg(unix)]
    fn guard_snapshot(directory: &Path) -> [(u64, u64, u32, Vec<u8>); 2] {
        use std::os::unix::fs::MetadataExt as _;
        [ControlFile::Access, ControlFile::Writer].map(|control| {
            let path = directory.join(control.file_name());
            let metadata = fs::metadata(&path).unwrap();
            (
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
                fs::read(path).unwrap(),
            )
        })
    }

    #[cfg(unix)]
    #[test]
    fn setup_normalizes_special_root_modes() {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        let scratch = Scratch::new();
        let graph = scratch.graph();
        prepare(&graph, true).unwrap();
        let controls = [ControlFile::Access, ControlFile::Writer];
        for control in controls {
            fs::write(
                graph.join(control.file_name()),
                control.file_name().as_bytes(),
            )
            .unwrap();
        }
        let guards = || guard_snapshot(&graph);
        let before = guards();
        fs::set_permissions(&graph, fs::Permissions::from_mode(0o1700)).unwrap();
        let mode = || fs::metadata(&graph).unwrap().permissions().mode() & 0o7777;
        let preview = prepare(&graph, false).unwrap();
        assert_eq!(
            (preview.action, preview.changed, mode()),
            ("secure_permissions", false, 0o1700)
        );
        assert!(preview.missing_guards.is_empty());
        assert_eq!(guards(), before);
        let applied = prepare(&graph, true).unwrap();
        assert_eq!(
            (applied.action, applied.changed, mode()),
            ("secure_permissions", true, 0o700)
        );
        assert!(applied.missing_guards.is_empty());
        assert_eq!(guards(), before);
        let ready_metadata = fs::metadata(&graph).unwrap();
        let ready = prepare(&graph, true).unwrap();
        assert_eq!((ready.action, ready.changed), ("ready", false));
        let after = fs::metadata(&graph).unwrap();
        assert_eq!(
            (after.ctime(), after.ctime_nsec()),
            (ready_metadata.ctime(), ready_metadata.ctime_nsec())
        );
        assert_eq!(guards(), before);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn setup_normalizes_special_root_modes_inherited_on_creation() {
        use std::os::unix::fs::PermissionsExt as _;
        let scratch = Scratch::new();
        let parent = scratch.0.join("setgid-parent");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o2700)).unwrap();
        let graph = parent.join(DIRECTORY);
        let parent_mode = || fs::metadata(&parent).unwrap().permissions().mode() & 0o7777;
        assert_eq!(parent_mode(), 0o2700);
        let preview = prepare(&graph, false).unwrap();
        assert_eq!(
            (preview.action, preview.changed),
            ("create_directory", false)
        );
        assert_eq!(parent_mode(), 0o2700);
        assert!(!graph.exists());
        let applied = prepare(&graph, true).unwrap();
        assert_eq!(
            (applied.action, applied.changed),
            ("create_directory", true)
        );
        assert_eq!(
            fs::metadata(&graph).unwrap().permissions().mode() & 0o7777,
            0o700
        );
        assert_eq!(parent_mode(), 0o2700);
        for control in [ControlFile::Access, ControlFile::Writer] {
            fs::write(
                graph.join(control.file_name()),
                control.file_name().as_bytes(),
            )
            .unwrap();
        }
        let before = guard_snapshot(&graph);
        let ready = prepare(&graph, true).unwrap();
        assert_eq!((ready.action, ready.changed), ("ready", false));
        assert_eq!(guard_snapshot(&graph), before);
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
