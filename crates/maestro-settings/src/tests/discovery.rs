//! Project-file discovery: the nearest `.maestro/config.toml` upward from the
//! working directory, never above home, never through a link.

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
