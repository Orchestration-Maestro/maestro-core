//! Tests for the file adapter's private platform operations.

use super::unix::Directory;
use rustix::fs::{Mode, mkfifoat};
use std::{
    error::Error,
    ffi::OsStr,
    fs, io,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Command,
    sync::mpsc,
    thread,
    time::Duration,
};

/// A scratch directory removed with its contents when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Result<Self, Box<dyn Error>> {
        Ok(Self(maestro_test_scratch::scratch_directory()?))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if fs::remove_dir_all(&self.0).is_err() {
            drop(fs::remove_file(&self.0));
        }
    }
}

#[test]
fn scratch_leaves_nothing_behind_as_a_directory_or_a_file() -> Result<(), Box<dyn Error>> {
    let directory = Scratch::new()?;
    fs::write(directory.0.join("preferences.toml"), "held")?;
    let directory_path = directory.0.clone();
    drop(directory);
    assert!(!directory_path.exists());

    let file = Scratch::new()?;
    fs::remove_dir(&file.0)?;
    fs::write(&file.0, "not a directory")?;
    let file_path = file.0.clone();
    drop(file);
    assert!(!file_path.exists());
    Ok(())
}

/// Opens a user directory with no child path.
fn open_directory(root: &Path) -> io::Result<Directory> {
    Directory::open_at(root, None, false)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "test directory disappeared"))
}

#[test]
fn private_file_descriptors_are_not_inherited_by_child_processes() -> Result<(), Box<dyn Error>> {
    let scratch = Scratch::new()?;
    let root = &scratch.0;
    let directory = open_directory(root)?;
    let _private = directory.create_private(OsStr::new("private"))?;
    let child = Command::new("sh")
        .args([
            "-c",
            "for fd in /proc/self/fd/*; do readlink \"$fd\" || true; done",
        ])
        .output()?;
    assert!(child.status.success());
    let descriptors = String::from_utf8_lossy(&child.stdout);
    assert!(
        !descriptors.contains(&root.display().to_string()),
        "{descriptors}"
    );
    Ok(())
}

#[test]
fn opening_a_non_directory_root_is_refused() -> Result<(), Box<dyn Error>> {
    let scratch = Scratch::new()?;
    fs::remove_dir_all(&scratch.0)?;
    fs::write(&scratch.0, "not a directory")?;
    assert!(Directory::open_at(&scratch.0, None, false).is_err());
    Ok(())
}

#[test]
fn opening_a_fifo_is_nonblocking_and_private_creation_is_exclusive() -> Result<(), Box<dyn Error>> {
    let scratch = Scratch::new()?;
    let root = &scratch.0;
    let root_file = fs::File::open(root)?;
    mkfifoat(&root_file, "pipe", Mode::RWXU)?;
    let directory = open_directory(root)?;
    let (finished, result) = mpsc::channel();
    thread::spawn(move || {
        drop(finished.send(directory.open_regular(OsStr::new("pipe"))));
    });
    assert!(matches!(
        result.recv_timeout(Duration::from_secs(1)),
        Ok(Err(_))
    ));

    let directory = open_directory(root)?;
    fs::write(root.join("preferences.toml"), "existing")?;
    assert!(matches!(
        directory.create_private(OsStr::new("preferences.toml")),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists
    ));
    symlink("target", root.join("linked"))?;
    assert!(matches!(
        directory.create_private(OsStr::new("linked")),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists
    ));
    Ok(())
}
