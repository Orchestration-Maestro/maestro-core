//! One edit of a preferences file: serialized by its lock, refused when
//! another program changed the file, private and permission-keeping, never
//! through a link, and undone only when the undo is confirmed.

use crate::{FileEdit, FileError, FileLayers, FilePlace, LayerSource as _, Registry};
use maestro_test_scratch::scratch_directory;
use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Barrier, mpsc},
    thread,
};

/// A scratch directory holding a project directory, removed when dropped.
struct Scratch(PathBuf);

impl Scratch {
    /// A new one.
    fn new() -> Self {
        let root = scratch_directory().unwrap().canonicalize().unwrap();
        fs::create_dir(root.join("project")).unwrap();
        Self(root)
    }

    /// The project file's place.
    fn place(&self) -> FilePlace {
        FilePlace::project(&self.file()).unwrap()
    }

    /// The project file.
    fn file(&self) -> PathBuf {
        self.0.join("project/.maestro/config.toml")
    }

    /// The project file's text as a session reads it, `Err` when refused.
    fn read(&self) -> Result<Option<String>, String> {
        let registry = Registry::built_in().unwrap();
        FileLayers::new(&self.0, Some(self.file()))
            .layers(&registry)
            .map(|layers| layers.project.map(|(_, layer)| format!("{layer:?}")))
            .map_err(|error| error.to_string())
    }

    /// The names in the project file's directory, sorted.
    fn names(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.file().parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

/// An edit of the scratch project file, its directory created.
fn begin(scratch: &Scratch) -> FileEdit {
    FileEdit::begin(scratch.place(), true, || {}).unwrap()
}

#[test]
fn an_edit_publishes_the_whole_file_and_keeps_only_the_file_and_its_lock() {
    let scratch = Scratch::new();
    let mut edit = begin(&scratch);
    assert_eq!(edit.before(), None);
    assert_eq!(edit.path(), scratch.file());
    edit.publish("schema = \"maestro-preferences/1\"\n")
        .unwrap();
    edit.keep();
    assert_eq!(scratch.read(), Ok(Some("Layer({})".to_owned())));
    fs::write(scratch.file(), "first\n").unwrap();

    let mut edit = begin(&scratch);
    assert_eq!(edit.before(), Some("first\n"));
    edit.publish("second\n").unwrap();
    assert_eq!(fs::read_to_string(edit.recovery_path()).unwrap(), "first\n");
    edit.keep();
    assert_eq!(fs::read_to_string(scratch.file()).unwrap(), "second\n");
    assert_eq!(scratch.names(), ["config.toml", "config.toml.lock"]);
}

#[test]
fn an_edit_that_would_create_nothing_creates_no_directory() {
    let scratch = Scratch::new();
    let edit = FileEdit::begin(scratch.place(), false, || {}).unwrap();
    assert_eq!(edit.before(), None);
    edit.keep();
    assert!(!scratch.0.join("project/.maestro").exists());
    assert_eq!(scratch.read(), Ok(None));
}

#[test]
fn a_second_edit_waits_for_the_first_to_end_and_reads_what_it_published() {
    let scratch = Arc::new(Scratch::new());
    let mut first = begin(&scratch);
    let (events, received) = mpsc::channel();
    let started = Arc::new(Barrier::new(2));
    let second = {
        let (scratch, started, events) =
            (Arc::clone(&scratch), Arc::clone(&started), events.clone());
        thread::spawn(move || {
            started.wait();
            let waited = events.clone();
            let mut second = FileEdit::begin(scratch.place(), true, move || {
                waited.send("waiting").unwrap();
            })
            .unwrap();
            events.send("began").unwrap();
            let before = second.before().map(str::to_owned);
            second.publish("second\n").unwrap();
            second.keep();
            before
        })
    };
    started.wait();
    assert_eq!(received.recv().unwrap(), "waiting", "the lock must hold it");
    first.publish("first\n").unwrap();
    first.keep();
    assert_eq!(received.recv().unwrap(), "began");
    assert_eq!(second.join().unwrap().as_deref(), Some("first\n"));
    assert_eq!(fs::read_to_string(scratch.file()).unwrap(), "second\n");
}

#[test]
fn a_file_another_program_changed_since_it_was_read_is_never_overwritten() {
    let scratch = Scratch::new();
    let mut edit = begin(&scratch);
    fs::write(scratch.file(), "theirs\n").unwrap();
    let refused = edit.publish("mine\n").unwrap_err();
    assert!(matches!(&refused, FileError::Changed(path) if *path == scratch.file()));
    assert!(refused.is_refusal());
    assert_eq!(
        refused.to_string(),
        format!(
            "{} changed while it was being edited, so nothing was written; run the command again",
            scratch.file().display()
        )
    );
    drop(edit);
    assert_eq!(fs::read_to_string(scratch.file()).unwrap(), "theirs\n");
    assert_eq!(scratch.names(), ["config.toml", "config.toml.lock"]);
}

#[test]
fn write_errors_are_not_file_refusals() {
    let scratch = Scratch::new();
    let mut original = begin(&scratch);
    original.publish("old\n").unwrap();
    original.keep();
    let mut edit = begin(&scratch);
    fs::create_dir(edit.recovery_path()).unwrap();
    let error = edit.publish("new\n").unwrap_err();
    assert!(matches!(error, FileError::Write { .. }));
    assert!(!error.is_refusal());
}

#[test]
fn undo_puts_back_the_old_file_or_removes_a_new_one() {
    let scratch = Scratch::new();
    let mut edit = begin(&scratch);
    edit.publish("new\n").unwrap();
    edit.undo().unwrap();
    assert!(!scratch.file().exists());

    fs::write(scratch.file(), "old\n").unwrap();
    let mut edit = begin(&scratch);
    edit.publish("new\n").unwrap();
    edit.undo().unwrap();
    assert_eq!(fs::read_to_string(scratch.file()).unwrap(), "old\n");
    assert_eq!(scratch.names(), ["config.toml", "config.toml.lock"]);
}

#[test]
fn an_undo_that_cannot_be_confirmed_names_the_recovery_copy() {
    let scratch = Scratch::new();
    fs::create_dir_all(scratch.file().parent().unwrap()).unwrap();
    fs::write(scratch.file(), "old\n").unwrap();
    let mut edit = begin(&scratch);
    edit.publish("new\n").unwrap();
    let recovery = edit.recovery_path();
    fs::write(scratch.file(), "theirs\n").unwrap();
    let error = edit.undo().unwrap_err();
    assert_eq!(error.recovery.as_deref(), Some(recovery.as_path()));
    assert_eq!(
        error.to_string(),
        format!(
            "{} could not be put back as it was (another program changed it since); its \
             previous text is kept in {}",
            scratch.file().display(),
            recovery.display()
        )
    );
    assert_eq!(fs::read_to_string(&recovery).unwrap(), "old\n");
    assert_eq!(fs::read_to_string(scratch.file()).unwrap(), "theirs\n");

    let mut edit = begin(&scratch);
    edit.publish("new\n").unwrap();
    fs::remove_file(scratch.file()).unwrap();
    fs::create_dir(scratch.file()).unwrap();
    let error = edit.undo().unwrap_err();
    assert_eq!(error.recovery.as_deref(), Some(recovery.as_path()));
    assert_eq!(fs::read_to_string(&recovery).unwrap(), "theirs\n");
}

#[test]
fn an_undo_of_a_new_file_that_cannot_be_removed_says_so() {
    let scratch = Scratch::new();
    let mut edit = begin(&scratch);
    edit.publish("new\n").unwrap();
    fs::write(scratch.file(), "theirs\n").unwrap();
    let error = edit.undo().unwrap_err();
    assert_eq!(error.recovery, None);
    assert!(
        error
            .to_string()
            .ends_with("it did not exist before, so remove it to undo the change"),
        "{error}"
    );
    assert_eq!(fs::read_to_string(scratch.file()).unwrap(), "theirs\n");
}

#[cfg(unix)]
#[test]
fn an_edit_keeps_the_file_s_permissions_and_creates_a_new_file_private() {
    use std::{os::unix::fs::PermissionsExt as _, path::Path};

    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
    let scratch = Scratch::new();
    let mut edit = begin(&scratch);
    edit.publish("new\n").unwrap();
    edit.keep();
    assert_eq!(mode(&scratch.file()), 0o600);
    fs::set_permissions(scratch.file(), fs::Permissions::from_mode(0o640)).unwrap();
    let mut edit = begin(&scratch);
    edit.publish("changed\n").unwrap();
    assert_eq!(mode(&edit.recovery_path()), 0o640);
    edit.undo().unwrap();
    assert_eq!(mode(&scratch.file()), 0o640);
    let mut edit = begin(&scratch);
    edit.publish("changed\n").unwrap();
    edit.keep();
    assert_eq!(mode(&scratch.file()), 0o640);
}

#[cfg(unix)]
#[test]
fn a_link_is_never_followed_to_read_or_write_the_file() {
    use std::os::unix::fs::symlink;

    let scratch = Scratch::new();
    let outside = scratch.0.join("outside");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("config.toml"), "theirs\n").unwrap();
    symlink(&outside, scratch.0.join("project/.maestro")).unwrap();
    for create in [false, true] {
        let refused = FileEdit::begin(scratch.place(), create, || {}).unwrap_err();
        assert!(refused.is_refusal());
        assert_eq!(
            refused.to_string(),
            format!(
                "cannot read {}: a link is never followed",
                scratch.file().display()
            )
        );
    }
    assert_eq!(
        scratch.read(),
        Err(format!(
            "cannot read {}: a link is never followed",
            scratch.file().display()
        ))
    );

    fs::remove_file(scratch.0.join("project/.maestro")).unwrap();
    fs::create_dir(scratch.0.join("project/.maestro")).unwrap();
    symlink(outside.join("config.toml"), scratch.file()).unwrap();
    assert!(FileEdit::begin(scratch.place(), true, || {}).is_err());
    assert_eq!(
        scratch.read(),
        Err(format!(
            "cannot read {}: a link is never followed",
            scratch.file().display()
        ))
    );
    let names: Vec<_> = fs::read_dir(&outside).unwrap().collect();
    assert_eq!(names.len(), 1);
    assert_eq!(
        fs::read_to_string(outside.join("config.toml")).unwrap(),
        "theirs\n"
    );
}
