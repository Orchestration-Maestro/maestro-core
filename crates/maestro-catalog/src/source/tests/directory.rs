//! The filesystem adapter: a bounded read, links never followed, and a
//! check that runs none of the catalog's scripts.

use super::support::{VALID, check_by};
use crate::{
    limits::Limits,
    source::{Directory, SourceTree, builtin},
};
use std::{env, fs, io, path::PathBuf, process};

/// A scratch directory, removed when dropped.
#[derive(Debug)]
struct Scratch {
    /// Its path.
    path: PathBuf,
}

impl Scratch {
    /// A new, empty scratch directory named after `name`.
    fn new(name: &str) -> Self {
        let path = env::temp_dir().join(format!("maestro-catalog-{name}-{}", process::id()));
        drop(fs::remove_dir_all(&path));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    /// Writes `text` at the relative `file`.
    fn write(&self, file: &str, text: &str) {
        let path = self.path.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.path));
    }
}

#[test]
fn directory_read_stops_one_byte_past_the_limit() {
    let scratch = Scratch::new("bounded-read");
    scratch.write("file.toml", "0123456789");
    let directory = Directory::new(&scratch.path);
    assert_eq!(directory.read("file.toml", 10).unwrap().len(), 10);
    assert_eq!(directory.read("file.toml", 9).unwrap().len(), 10);
    assert_eq!(directory.read("file.toml", 4).unwrap().len(), 5);
}

#[test]
fn directory_checks_the_valid_catalog_without_running_its_scripts() {
    let scratch = Scratch::new("valid");
    for (file, text) in VALID {
        scratch.write(file, text);
    }
    let marker = scratch.path.join("ran");
    scratch.write(
        "skills/valid-skill/scripts/run.sh",
        &format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
    );
    let catalog = check_by(
        &Directory::new(&scratch.path),
        &builtin().unwrap(),
        &Limits::PRODUCTION,
    );
    assert_eq!(catalog.map(|catalog| catalog.resources.len()), Ok(5));
    assert!(!marker.exists());
}

// Creating a symbolic link on Windows needs a privilege CI runners may lack;
// the listing code is shared, so Unix covers it.
#[cfg(unix)]
#[test]
fn directory_lists_a_link_as_unsupported() {
    use crate::source::EntryKind;
    use std::os::unix::fs::symlink;

    let scratch = Scratch::new("link");
    scratch.write("agents/base/valid.agent.md", "text");
    symlink(
        scratch.path.join("agents/base/valid.agent.md"),
        scratch.path.join("agents/base/linked.agent.md"),
    )
    .unwrap();
    let entries = Directory::new(&scratch.path).list("agents/base").unwrap();
    assert_eq!(entries[0].name, "linked.agent.md");
    assert_eq!(entries[0].kind, EntryKind::Unsupported);
    assert_eq!(entries[1].kind, EntryKind::File);
}

#[test]
fn directory_refuses_paths_that_leave_or_bypass_the_root() {
    let scratch = Scratch::new("escape");
    scratch.write("inside/file.toml", "inside");
    let outside = Scratch::new("escape-outside");
    outside.write("file.toml", "outside");
    let escape = format!(
        "../{}/file.toml",
        outside.path.file_name().unwrap().to_str().unwrap()
    );
    let directory = Directory::new(&scratch.path);
    let absolute = outside.path.join("file.toml");
    for path in [
        escape.as_str(),
        "inside/../inside/file.toml",
        "./inside/file.toml",
        "inside//file.toml",
        "inside\\file.toml",
        "c:/file.toml",
        absolute.to_str().unwrap(),
    ] {
        let error = directory.read(path, 100).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{path}");
        let error = directory.list(path).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{path}");
    }
    assert_eq!(directory.read("inside/file.toml", 100).unwrap(), b"inside");
}
