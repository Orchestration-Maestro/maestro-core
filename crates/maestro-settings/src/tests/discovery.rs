//! Project-file discovery: the nearest `.maestro/config.toml` upward from the
//! working directory, never above home, never through a link.

use std::io::ErrorKind;

use crate::{PROJECT_DIRECTORY, PROJECT_FILE, discover_project_file};
use maestro_test_scratch::scratch_directory;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// A scratch directory standing for the home, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    /// A new scratch directory with `home/` in it.
    fn new() -> Self {
        let root = scratch_directory().unwrap();
        fs::create_dir_all(root.join("home")).unwrap();
        Self(root.canonicalize().unwrap())
    }

    /// The home.
    fn home(&self) -> PathBuf {
        self.0.join("home")
    }

    /// `relative` under the home, created as a directory.
    fn directory(&self, relative: &str) -> PathBuf {
        let path = self.home().join(relative);
        fs::create_dir_all(&path).unwrap();
        path
    }

    /// Plants a project file in `directory`, returning its path.
    fn plant(directory: &Path) -> PathBuf {
        let folder = directory.join(PROJECT_DIRECTORY);
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join(PROJECT_FILE);
        fs::write(&file, "schema = \"maestro-preferences/1\"\n").unwrap();
        file
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

#[test]
fn discover_project_file_finds_the_nearest_file_up_to_home_inclusive() {
    let scratch = Scratch::new();
    let start = scratch.directory("work/project/src/deep");
    let outer = Scratch::plant(&scratch.home());
    let found = discover_project_file(&start, Some(&scratch.home()));
    assert_eq!(found.file.as_deref(), Some(outer.as_path()));
    assert_eq!(found.note, None);
    assert!(found.skipped.is_empty());

    let project = Scratch::plant(&scratch.directory("work/project"));
    fs::create_dir_all(
        scratch
            .directory("work/project/src")
            .join(PROJECT_DIRECTORY),
    )
    .unwrap();
    let found = discover_project_file(&start, Some(&scratch.home()));
    assert_eq!(found.file.as_deref(), Some(project.as_path()));

    let found = discover_project_file(
        &scratch.directory("work/project/src"),
        Some(&scratch.home()),
    );
    assert_eq!(found.file.as_deref(), Some(project.as_path()));
    assert!(found.skipped.is_empty());
}

#[test]
fn discover_project_file_reads_nothing_above_home_or_outside_it() {
    let scratch = Scratch::new();
    Scratch::plant(&scratch.0);
    let start = scratch.directory("work");
    let found = discover_project_file(&start, Some(&scratch.home()));
    assert_eq!(found.file, None);
    assert_eq!(found.note, None);

    let outside = scratch.0.join("elsewhere");
    fs::create_dir_all(&outside).unwrap();
    Scratch::plant(&outside);
    let found = discover_project_file(&outside, Some(&scratch.home()));
    assert_eq!(found.file, None);
    assert_eq!(
        found.note.as_deref(),
        Some("the directory is outside the home directory: no project file is read")
    );
    let found = discover_project_file(&outside, None);
    assert_eq!(found.file, None);
    assert_eq!(
        found.note.as_deref(),
        Some("no home directory is known: no project file is read")
    );
    let found = discover_project_file(&scratch.0.join("missing"), Some(&scratch.home()));
    assert_eq!(found.file, None);
    assert!(
        found
            .note
            .as_deref()
            .is_some_and(|note| note.starts_with("the directory cannot be resolved: ")),
        "{found:?}"
    );
}

#[test]
fn discover_project_file_skips_a_project_file_that_is_not_a_file() {
    let scratch = Scratch::new();
    let outer = Scratch::plant(&scratch.home());
    let inner = scratch.directory("inner");
    fs::create_dir_all(inner.join(PROJECT_DIRECTORY).join(PROJECT_FILE)).unwrap();
    let found = discover_project_file(&inner, Some(&scratch.home()));
    assert_eq!(found.file.as_deref(), Some(outer.as_path()));
    assert_eq!(
        found.skipped,
        vec![format!(
            "{}: skipped: not a regular file",
            inner.join(PROJECT_DIRECTORY).join(PROJECT_FILE).display()
        )]
    );
}

#[cfg(unix)]
#[test]
fn discover_project_file_never_follows_a_link() {
    use std::os::unix::fs::symlink;

    let scratch = Scratch::new();
    let outer = Scratch::plant(&scratch.home());
    let elsewhere = Scratch::plant(&scratch.0);
    let linked_folder = scratch.directory("linked_folder");
    symlink(
        elsewhere.parent().unwrap(),
        linked_folder.join(PROJECT_DIRECTORY),
    )
    .unwrap();
    let linked_file = scratch.directory("linked_folder/linked_file");
    fs::create_dir_all(linked_file.join(PROJECT_DIRECTORY)).unwrap();
    symlink(
        &elsewhere,
        linked_file.join(PROJECT_DIRECTORY).join(PROJECT_FILE),
    )
    .unwrap();
    let found = discover_project_file(&linked_file, Some(&scratch.home()));
    assert_eq!(found.file.as_deref(), Some(outer.as_path()));
    assert_eq!(
        found.skipped,
        vec![
            format!(
                "{}: skipped: a link is never followed",
                linked_file
                    .join(PROJECT_DIRECTORY)
                    .join(PROJECT_FILE)
                    .display()
            ),
            format!(
                "{}: skipped: a link is never followed",
                linked_folder.join(PROJECT_DIRECTORY).display()
            ),
        ]
    );
}

#[test]
fn callback_walk_respects_boundary_and_returns_nearest_held_snapshot() {
    use crate::discover_project_with;
    use maestro_filesystem::Directory;
    let root = scratch_directory().unwrap();
    let boundary = root.join("home");
    let start = boundary.join("nested/deeper");
    fs::create_dir_all(&start).unwrap();
    fs::write(boundary.join("value"), "farther").unwrap();
    fs::write(boundary.join("nested/value"), "nearest").unwrap();
    let start = start.canonicalize().unwrap();
    let boundary = boundary.canonicalize().unwrap();
    let mut held = Directory::open_canonical(&start).unwrap();
    let mut visited = Vec::new();
    let (discovery, snapshot) = discover_project_with(&start, &boundary, |path| {
        visited.push(path.to_path_buf());
        let candidate = match held.read_regular_bounded("value", 100) {
            Ok(bytes) => Some((path.join("value"), bytes)),
            Err(error) if error.kind() == ErrorKind::NotFound => None,
            Err(error) => panic!("{error}"),
        };
        held = held.parent().unwrap();
        Ok(candidate)
    });
    assert_eq!(snapshot.unwrap(), b"nearest");
    assert_eq!(discovery.file, Some(boundary.join("nested/value")));
    assert_eq!(visited, [start.clone(), boundary.join("nested")]);
    let mut count = 0;
    let (missing, snapshot) = discover_project_with(&start, &boundary, |_| {
        count += 1;
        Ok::<_, String>(None::<(PathBuf, Vec<u8>)>)
    });
    assert_eq!(count, 3);
    assert!(missing.file.is_none() && snapshot.is_none());
    let (outside, _) = discover_project_with(
        &root,
        &boundary,
        |_| -> Result<Option<(PathBuf, Vec<u8>)>, String> {
            panic!("a callback outside the approved boundary must never run")
        },
    );
    assert!(outside.note.is_some());
    drop(held);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn held_callback_refuses_an_ancestor_swap_and_never_reopens_selected_bytes() {
    use crate::discover_project_with;
    use maestro_filesystem::Directory;
    let root = scratch_directory().unwrap();
    let start = root.join("home/nested");
    fs::create_dir_all(&start).unwrap();
    fs::write(start.join("config"), "original").unwrap();
    let start = start.canonicalize().unwrap();
    let boundary = root.join("home").canonicalize().unwrap();
    let held = Directory::open_canonical(&start).unwrap();
    let (discovery, _) = discover_project_with(&start, &boundary, |path| {
        if path == start {
            fs::rename(&start, boundary.join("displaced")).unwrap();
            fs::create_dir(&start).unwrap();
            fs::write(start.join("config"), "planted").unwrap();
            return held
                .read_preferences("config", 100)
                .map(|bytes| Some((path.join("config"), bytes)))
                .map_err(|error| format!("skipped: {error}"));
        }
        Ok(None)
    });
    assert!(discovery.file.is_none());
    assert!(discovery.skipped[0].contains("changed"));
    // A selected callback snapshot remains its original bytes after an edit.
    let held = Directory::open_canonical(&start).unwrap();
    let (_, snapshot) = discover_project_with(&start, &boundary, |path| {
        let bytes = held.read_regular_bounded("config", 100).unwrap();
        fs::write(start.join("config"), "later edit").unwrap();
        Ok(Some((path.join("config"), bytes)))
    });
    assert_eq!(snapshot.unwrap(), b"planted");
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn held_callback_prevents_ancestor_swap_on_windows() {
    use crate::discover_project_with;
    use maestro_filesystem::Directory;
    let root = scratch_directory().unwrap();
    let start = root.join("home/nested");
    fs::create_dir_all(&start).unwrap();
    fs::write(start.join("config"), "original").unwrap();
    let start = start.canonicalize().unwrap();
    let boundary = root.join("home").canonicalize().unwrap();
    let held = Directory::open_canonical(&start).unwrap();
    let (_, snapshot) = discover_project_with(&start, &boundary, |path| {
        assert!(fs::rename(&boundary, root.join("displaced")).is_err());
        let bytes = held.read_regular_bounded("config", 100).unwrap();
        fs::write(start.join("config"), "later edit").unwrap();
        Ok(Some((path.join("config"), bytes)))
    });
    assert_eq!(snapshot.unwrap(), b"original");
    drop(held);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn callback_discovery_refuses_relative_start_or_boundary_without_visiting() {
    use crate::discover_project_with;
    let scratch = scratch_directory().unwrap();
    let absolute = scratch.canonicalize().unwrap();
    for (start, boundary) in [
        (Path::new("relative"), absolute.as_path()),
        (absolute.as_path(), Path::new("")),
    ] {
        let mut visits = 0;
        let (discovery, snapshot) = discover_project_with(start, boundary, |_| {
            visits += 1;
            Ok(Some((PathBuf::from("selected"), ())))
        });
        assert_eq!(visits, 0);
        assert!(snapshot.is_none());
        assert_eq!(
            discovery.note.as_deref(),
            Some("directory is outside the approved boundary")
        );
    }
    fs::remove_dir_all(scratch).unwrap();
}
