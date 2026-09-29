//! `config set` and `config unset`: the file they choose, the edit, and the
//! journal record every landed change gets, or the file left as it was.

use super::{
    Change, Places, change as run,
    change::{run_with, target},
};
use crate::{cli::output::Output, failure::Failure, kernel::Kernel};
use maestro_kernel::{scope::LOCAL, settings::SettingChange};
use maestro_settings::{
    FileEdit, FilePlace, LayerName, PROJECT_DIRECTORY, PROJECT_FILE, USER_FILE,
};
use maestro_test_scratch::scratch_directory;
use rusqlite::Connection;
use serde_json::json;
use std::{fs, path::PathBuf, sync::mpsc, thread};

/// A scratch home with `config/`, `data/` and `home/work/`, removed when
/// dropped.
struct Scratch(PathBuf);

impl Scratch {
    /// A new one.
    fn new() -> Self {
        let root = scratch_directory().unwrap().canonicalize().unwrap();
        for directory in ["config", "data", "home/work"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        Self(root)
    }

    /// Where the change commands look.
    fn places(&self) -> Places {
        Places {
            config_dir: self.0.join("config"),
            working: Some(self.0.join("home").join("work")),
            home: Some(self.0.join("home")),
        }
    }

    /// The user file.
    fn user_file(&self) -> PathBuf {
        self.0.join("config").join(USER_FILE)
    }

    /// The kernel of the scratch directories.
    fn kernel(&self) -> Result<Kernel, Failure> {
        Kernel::open_at(&self.0.join("data"), &self.0.join("config"))
    }

    /// Applies `change`.
    fn change(&self, key: &str, value: Option<&str>, layer: LayerName) -> Result<(), String> {
        let change = Change { key, value, layer };
        run(Output::new(true), change, &self.places(), || self.kernel())
            .map(drop)
            .map_err(|failure| failure.to_string())
    }

    /// Applies `tone = brief` in the user file, recording it with `journal`.
    fn change_with(
        &self,
        journal: impl FnOnce(&Kernel, &SettingChange) -> Result<(), String>,
    ) -> Result<(), String> {
        let change = Change {
            key: "tone",
            value: Some("brief"),
            layer: LayerName::User,
        };
        run_with(
            Output::new(true),
            change,
            &self.places(),
            || self.kernel(),
            journal,
        )
        .map(drop)
        .map_err(|failure| failure.to_string())
    }

    /// The journaled changes, each as `(key, old, new, layer, file)`.
    fn journal(&self) -> Vec<serde_json::Value> {
        self.kernel()
            .unwrap()
            .database
            .setting_changes(LOCAL)
            .unwrap()
            .into_iter()
            .map(|recorded| {
                let change = recorded.change;
                json!([
                    change.key,
                    change.old,
                    change.new,
                    change.layer,
                    change.file
                ])
            })
            .collect()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

#[test]
fn target_is_the_user_file_or_the_nearest_project_file_within_home() {
    let scratch = Scratch::new();
    let places = scratch.places();
    assert_eq!(
        target(LayerName::User, &places).unwrap(),
        scratch.user_file()
    );
    let new = scratch
        .0
        .join("home/work")
        .join(PROJECT_DIRECTORY)
        .join(PROJECT_FILE);
    assert_eq!(target(LayerName::Project, &places).unwrap(), new);
    let outer = scratch
        .0
        .join("home")
        .join(PROJECT_DIRECTORY)
        .join(PROJECT_FILE);
    fs::create_dir_all(outer.parent().unwrap()).unwrap();
    fs::write(&outer, "schema = \"maestro-preferences/1\"\n").unwrap();
    assert_eq!(target(LayerName::Project, &places).unwrap(), outer);
    let outside = Places {
        working: Some(scratch.0.clone()),
        ..places.clone()
    };
    assert_eq!(
        target(LayerName::Project, &outside)
            .unwrap_err()
            .to_string(),
        "--project: the directory is outside the home directory: no project file is read"
    );
    let unknown = Places {
        working: None,
        ..places
    };
    assert_eq!(
        target(LayerName::Project, &unknown)
            .unwrap_err()
            .to_string(),
        "--project: the working directory is unknown"
    );
}

#[test]
fn set_writes_the_value_and_journals_each_change_once() {
    let scratch = Scratch::new();
    scratch
        .change("tone", Some("brief"), LayerName::User)
        .unwrap();
    assert_eq!(
        fs::read_to_string(scratch.user_file()).unwrap(),
        "schema = \"maestro-preferences/1\"\ntone = \"brief\"\n"
    );
    scratch
        .change("tone", Some("brief"), LayerName::User)
        .unwrap();
    scratch
        .change("tone", Some("detailed"), LayerName::User)
        .unwrap();
    let file = scratch.user_file().display().to_string();
    assert_eq!(
        scratch.journal(),
        vec![
            json!(["tone", null, "brief", "user", file]),
            json!(["tone", "brief", "detailed", "user", file]),
        ]
    );
}

#[test]
fn unset_removes_the_line_and_journals_it_and_a_missing_key_changes_nothing() {
    let scratch = Scratch::new();
    fs::write(
        scratch.user_file(),
        "schema = \"maestro-preferences/1\"\n# mine\nsearch.k = 20\ntone = \"brief\"\n",
    )
    .unwrap();
    scratch.change("search.k", None, LayerName::User).unwrap();
    assert_eq!(
        fs::read_to_string(scratch.user_file()).unwrap(),
        "schema = \"maestro-preferences/1\"\n# mine\ntone = \"brief\"\n"
    );
    scratch.change("search.k", None, LayerName::User).unwrap();
    scratch
        .change("language", None, LayerName::Project)
        .unwrap();
    assert!(!scratch.0.join("home/work").join(PROJECT_DIRECTORY).exists());
    let file = scratch.user_file().display().to_string();
    assert_eq!(
        scratch.journal(),
        vec![json!(["search.k", 20, null, "user", file])]
    );
}

#[test]
fn set_in_the_project_creates_the_project_file_in_the_working_directory() {
    let scratch = Scratch::new();
    scratch
        .change("search.rerank.depth", Some("40"), LayerName::Project)
        .unwrap();
    let file = scratch
        .0
        .join("home/work")
        .join(PROJECT_DIRECTORY)
        .join(PROJECT_FILE);
    assert_eq!(
        fs::read_to_string(&file).unwrap(),
        "schema = \"maestro-preferences/1\"\n\n[search.rerank]\ndepth = 40\n"
    );
    assert_eq!(
        scratch.journal(),
        vec![json!([
            "search.rerank.depth",
            null,
            40,
            "project",
            file.display().to_string()
        ])]
    );
    assert!(!scratch.user_file().exists());
}

#[test]
fn a_refused_key_value_or_file_changes_and_journals_nothing() {
    let scratch = Scratch::new();
    assert_eq!(
        scratch.change("nothing", Some("1"), LayerName::User),
        Err("unknown key \"nothing\": `maestro config list` names every setting".to_owned())
    );
    assert_eq!(
        scratch.change("search.k", Some("99"), LayerName::User),
        Err("search.k: expected a whole number from 1 to 50".to_owned())
    );
    assert_eq!(
        scratch.change("models.compute", Some("cpu"), LayerName::User),
        Err("models.compute: cpu mode comes after M1".to_owned())
    );
    let invalid = "schema = \"maestro-preferences/1\"\n[access]\nread = []\n";
    fs::write(scratch.user_file(), invalid).unwrap();
    assert_eq!(
        scratch.change("tone", Some("brief"), LayerName::User),
        Err(format!(
            "{}: unknown key \"access\"",
            scratch.user_file().display()
        ))
    );
    assert_eq!(fs::read_to_string(scratch.user_file()).unwrap(), invalid);
    assert!(scratch.journal().is_empty());
}

#[test]
fn a_change_the_journal_refuses_leaves_the_file_as_it_was() {
    let scratch = Scratch::new();
    drop(scratch.kernel().unwrap());
    Connection::open(scratch.0.join("data").join("kernel.sqlite3"))
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER refuse_settings BEFORE INSERT ON events
             WHEN NEW.type = 'maestro.kernel.setting.changed.v1'
             BEGIN SELECT RAISE(ABORT, 'refused'); END;",
        )
        .unwrap();
    let error = scratch
        .change("tone", Some("brief"), LayerName::User)
        .unwrap_err();
    assert!(
        error.starts_with(&format!(
            "the change of tone could not be journaled, so {} is left as it was",
            scratch.user_file().display()
        )),
        "{error}"
    );
    assert!(!scratch.user_file().exists());
    let before = "schema = \"maestro-preferences/1\"\ntone = \"normal\"\n";
    fs::write(scratch.user_file(), before).unwrap();
    assert!(
        scratch
            .change("tone", Some("brief"), LayerName::User)
            .is_err()
    );
    assert_eq!(fs::read_to_string(scratch.user_file()).unwrap(), before);
}

#[cfg(unix)]
#[test]
fn a_project_change_never_follows_a_link_out_of_the_project() {
    use std::os::unix::fs::symlink;

    let scratch = Scratch::new();
    let outside = scratch.0.join("outside");
    fs::create_dir(&outside).unwrap();
    let folder = scratch.0.join("home/work").join(PROJECT_DIRECTORY);
    symlink(&outside, &folder).unwrap();
    let refused = scratch
        .change("tone", Some("brief"), LayerName::Project)
        .unwrap_err();
    assert!(refused.contains("a link is never followed"), "{refused}");
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);

    fs::remove_file(&folder).unwrap();
    fs::create_dir(&folder).unwrap();
    let target = outside.join("theirs.toml");
    let theirs = "schema = \"maestro-preferences/1\"\n";
    fs::write(&target, theirs).unwrap();
    symlink(&target, folder.join(PROJECT_FILE)).unwrap();
    let refused = scratch
        .change("tone", Some("brief"), LayerName::Project)
        .unwrap_err();
    assert!(refused.contains("a link is never followed"), "{refused}");
    assert_eq!(fs::read_to_string(&target).unwrap(), theirs);
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
    assert!(scratch.journal().is_empty());
}

#[cfg(unix)]
#[test]
fn a_change_keeps_a_private_file_private() {
    use std::os::unix::fs::PermissionsExt as _;

    let scratch = Scratch::new();
    fs::write(scratch.user_file(), "schema = \"maestro-preferences/1\"\n").unwrap();
    fs::set_permissions(scratch.user_file(), fs::Permissions::from_mode(0o600)).unwrap();
    scratch
        .change("tone", Some("brief"), LayerName::User)
        .unwrap();
    let mode = fs::metadata(scratch.user_file())
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    scratch
        .change("language", Some("fr"), LayerName::Project)
        .unwrap();
    let created = scratch
        .0
        .join("home/work")
        .join(PROJECT_DIRECTORY)
        .join(PROJECT_FILE);
    let mode = fs::metadata(created).unwrap().permissions().mode();
    assert_eq!(mode & 0o077, 0, "a new file is private: {mode:o}");
}

#[test]
fn a_journal_failure_whose_undo_cannot_be_confirmed_names_the_recovery_copy() {
    let scratch = Scratch::new();
    let before = "schema = \"maestro-preferences/1\"\ntone = \"normal\"\n";
    fs::write(scratch.user_file(), before).unwrap();
    let file = scratch.user_file();
    let error = scratch
        .change_with(|_, _| {
            fs::write(&file, "theirs\n").unwrap();
            Err("refused".to_owned())
        })
        .unwrap_err();
    let recovery = scratch.0.join("config").join("preferences.toml.previous");
    assert_eq!(
        error,
        format!(
            "the change of tone could not be journaled (refused), and {} could not be put back \
             as it was (another program changed it since); its previous text is kept in {}",
            file.display(),
            recovery.display()
        )
    );
    assert_eq!(fs::read_to_string(&recovery).unwrap(), before);
    assert!(!error.contains("left as it was"));
}

#[test]
fn another_change_waits_through_the_journal_and_the_undo() {
    let scratch = Scratch::new();
    let before = "schema = \"maestro-preferences/1\"\ntone = \"normal\"\n";
    fs::write(scratch.user_file(), before).unwrap();
    let (waiting, waited) = mpsc::channel();
    let place = FilePlace::user(&scratch.0.join("config"));
    let mut other = None;
    let error = scratch
        .change_with(|_, _| {
            other = Some(thread::spawn(move || {
                FileEdit::begin(place, false, move || {
                    waiting.send(()).unwrap();
                })
                .unwrap()
                .before()
                .map(str::to_owned)
            }));
            waited.recv().unwrap();
            Err("refused".to_owned())
        })
        .unwrap_err();
    assert!(error.contains("is left as it was"), "{error}");
    let read = other.unwrap().join().unwrap();
    assert_eq!(read.as_deref(), Some(before));
}
