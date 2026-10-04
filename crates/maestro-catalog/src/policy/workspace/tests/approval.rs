//! Default-no IO, adapter floor, and private digest receipts.
use super::paths::literal_suffix;
use crate::{
    files::FileInput,
    policy::workspace::{
        CheckedTrust, JournalTrust, TrustBoundaries, WorkspaceTrust, confirmation,
        write_preferences,
    },
};
use maestro_filesystem::Directory;
use maestro_kernel::{
    artifact::Digest,
    store::Database,
    workspace::{Answer, Confirmation, WorkspaceAnswer},
};
use maestro_test_scratch::scratch_directory;
use std::{
    fs,
    path::{Path, PathBuf},
    slice::from_ref,
};

struct SuppliedRoot(PathBuf);
impl WorkspaceTrust for SuppliedRoot {
    fn containing_root(&self, _start: &Path) -> Option<PathBuf> {
        Some(self.0.clone())
    }
}

#[test]
fn terminal_defaults_no_and_eof_cannot_approve() {
    let path = Path::new("/synthetic/workspace");
    for answer in [
        "",
        "\n",
        "no\n",
        "catalog says yes\n",
        "{\"approved\":true}\n",
    ] {
        let mut output = Vec::new();
        assert_eq!(
            confirmation(
                path,
                None,
                true,
                "Approve synthetic? [y/N] ",
                (&mut answer.as_bytes(), &mut output)
            )
            .unwrap(),
            None
        );
        assert!(String::from_utf8(output).unwrap().contains("[y/N]"));
    }
    assert_eq!(
        confirmation(
            path,
            None,
            true,
            "Approve synthetic? [y/N] ",
            (&mut "yes\n".as_bytes(), &mut Vec::new())
        )
        .unwrap(),
        Some(Confirmation::Terminal)
    );
    assert!(
        confirmation(
            path,
            None,
            false,
            "Approve synthetic? [y/N] ",
            (&mut "yes\n".as_bytes(), &mut Vec::new())
        )
        .is_err()
    );
}

#[test]
fn confirm_path_rejects_relative_dot_and_mismatched_spellings() {
    let scratch = scratch_directory().unwrap();
    let canonical = scratch.canonicalize().unwrap();
    for path in [
        literal_suffix(&canonical, "."),
        PathBuf::from("workspace"),
        canonical.join("child"),
    ] {
        assert!(
            confirmation(
                &canonical,
                Some(&path),
                false,
                "unused prompt",
                (&mut &b""[..], &mut Vec::new())
            )
            .is_err()
        );
    }
    assert_eq!(
        confirmation(
            &canonical,
            Some(&canonical),
            false,
            "unused prompt",
            (&mut &b""[..], &mut Vec::new())
        )
        .unwrap(),
        Some(Confirmation::ConfirmPath)
    );
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn every_adapter_keeps_home_internal_and_root_refusals() {
    let scratch = scratch_directory().unwrap();
    let home = scratch.canonicalize().unwrap();
    let ordinary = home.join("project");
    let internal = home.join("kernel");
    fs::create_dir(&ordinary).unwrap();
    fs::create_dir(&internal).unwrap();
    let boundaries = TrustBoundaries::new(&home, from_ref(&internal)).unwrap();
    for denied in [&home, &internal, home.ancestors().last().unwrap()] {
        assert!(boundaries.canonical_root(denied).is_err());
        assert!(
            CheckedTrust::new(&SuppliedRoot(denied.to_path_buf()), &boundaries)
                .containing_root(denied)
                .is_none()
        );
    }
    assert!(
        CheckedTrust::new(&SuppliedRoot(ordinary.clone()), &boundaries)
            .containing_root(&home)
            .is_none()
    );
    assert_eq!(
        CheckedTrust::new(&SuppliedRoot(ordinary.clone()), &boundaries).containing_root(&ordinary),
        Some(ordinary)
    );
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn held_mount_or_drive_root_refuses_beside_ordinary_directory() {
    let scratch = scratch_directory().unwrap();
    let root = scratch.canonicalize().unwrap();
    let boundaries = TrustBoundaries::new(&root.join(".."), &[]).unwrap();
    let mount = root
        .ancestors()
        .find(|path| {
            Directory::open_canonical(path)
                .unwrap()
                .is_mount_root()
                .unwrap()
        })
        .unwrap();
    assert!(boundaries.canonical_root(mount).is_err());
    assert_eq!(boundaries.canonical_root(&root).unwrap(), root);
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn journal_revocation_is_observed_by_the_same_adapter() {
    let scratch = scratch_directory().unwrap();
    let database = Database::open_in(&scratch.join("kernel")).unwrap();
    let root = scratch.canonicalize().unwrap().join("project");
    fs::create_dir(&root).unwrap();
    let record = |answer| {
        database
            .record_workspace_answer(&WorkspaceAnswer {
                path: root.clone(),
                answer,
            })
            .unwrap()
    };
    let adapter = JournalTrust::new(&database);
    assert!(adapter.containing_root(&root).is_none());
    let approved = record(Answer::Approved {
        confirmation: Confirmation::ConfirmPath,
    });
    assert_eq!(adapter.containing_root(&root), Some(root.clone()));
    assert_eq!(database.trusted_workspaces().unwrap(), vec![approved]);
    record(Answer::Removed);
    assert!(adapter.containing_root(&root).is_none());
    drop(database);
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn declined_preferences_write_only_config_and_internal_digest_receipts() {
    let scratch = scratch_directory().unwrap();
    let database = Database::open_in(&scratch.join("kernel")).unwrap();
    let root = scratch.canonicalize().unwrap().join("project");
    fs::create_dir(&root).unwrap();
    let file = FileInput::new(
        ".maestro/config.toml",
        b"schema = 'maestro-preferences/1'\nlanguage = 'fr'\n".to_vec(),
    );
    let boundaries = TrustBoundaries::new(root.parent().unwrap(), &[]).unwrap();
    let adapter = JournalTrust::new(&database);
    let checked = CheckedTrust::new(&adapter, &boundaries);
    let permit = || {
        super::super::preferences_confirmation(
            &root,
            Some(&root),
            false,
            "",
            (&mut &b""[..], &mut Vec::new()),
        )
        .unwrap()
        .unwrap()
    };
    let other = FileInput::new("template.md", b"unapproved".to_vec());
    assert!(write_preferences(&database, &checked, &other, permit()).is_err());
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    assert!(database.workspace_answers().unwrap().is_empty());
    write_preferences(&database, &checked, &file, permit()).unwrap();
    assert_eq!(fs::read(root.join(&file.path)).unwrap(), file.bytes);
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    assert_eq!(fs::read_dir(root.join(".maestro")).unwrap().count(), 1);
    let answers = database.workspace_answers().unwrap();
    assert_eq!(answers.len(), 3);
    assert_eq!(answers[0].change.answer, Answer::Declined);
    assert_eq!(
        answers[2].change.answer,
        Answer::Preferences {
            digest: format!("sha256:{}", Digest::of(&file.bytes).as_str()),
            confirmation: Confirmation::ConfirmPath,
            completed: true
        }
    );
    assert!(database.trusted_workspaces().unwrap().is_empty());
    assert!(!root.join("template.md").exists());
    assert!(write_preferences(&database, &checked, &file, permit()).is_err());
    assert_eq!(fs::read(root.join(&file.path)).unwrap(), file.bytes);
    drop(database);
    fs::remove_dir_all(scratch).unwrap();
}

#[test]
fn preferences_capability_floor_is_one_file_one_root_never_home_or_alias() {
    let home = scratch_directory().unwrap().canonicalize().unwrap();
    let root = home.join("project");
    let other = home.join("other");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&other).unwrap();
    let boundaries = TrustBoundaries::new(&home, &[]).unwrap();
    let adapter = JournalTrust::optional(None);
    assert!(CheckedTrust::preferences(&adapter, &boundaries, &home).is_err());
    let narrow = CheckedTrust::preferences(&adapter, &boundaries, &root).unwrap();
    for path in [
        "sibling",
        ".maestro/other",
        "../config.toml",
        ".maestro/../config.toml",
    ] {
        assert!(narrow.authorize_create(&root, Path::new(path)).is_err());
    }
    assert!(
        narrow
            .authorize_create(&other, Path::new(".maestro/config.toml"))
            .is_err()
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    assert_eq!(fs::read_dir(&other).unwrap().count(), 0);
    let alias = home.join("alias");
    super::paths::directory_link(&root, &alias);
    assert!(
        narrow
            .authorize_create(&alias, Path::new(".maestro/config.toml"))
            .is_err()
    );
    #[cfg(unix)]
    {
        fs::rename(&root, home.join("original")).unwrap();
        super::paths::directory_link(&other, &root);
        assert!(
            narrow
                .authorize_create(&root, Path::new(".maestro/config.toml"))
                .is_err()
        );
        assert_eq!(fs::read_dir(&other).unwrap().count(), 0);
    }
    drop(narrow);
    fs::remove_dir_all(home).unwrap();
}
