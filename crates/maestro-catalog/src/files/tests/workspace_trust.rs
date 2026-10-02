//! Owned effects must not turn a preview or ownership record into path authority.
use super::support::Scratch;
use crate::{
    files::{FileInput, FilePlan, apply, apply_with_failure, recover, remove},
    policy::workspace::{Access, CheckedTrust, TrustBoundaries, WorkspaceTrust},
};
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::process::Command;
use std::{
    cell::Cell,
    fs,
    path::{Path, PathBuf},
};

/// Revocable synthetic authority models the journal without granting kernel storage.
struct Fixture {
    scratch: Scratch,
    root: PathBuf,
    boundaries: TrustBoundaries,
    approved: Cell<bool>,
}
impl Fixture {
    fn new() -> Self {
        let scratch = Scratch::new();
        let home = scratch.path.canonicalize().unwrap();
        let root = home.join("project");
        fs::create_dir(&root).unwrap();
        fs::create_dir(home.join("kernel")).unwrap();
        let boundaries = TrustBoundaries::new(&home, &[home.join("kernel")]).unwrap();
        Self {
            scratch,
            root,
            boundaries,
            approved: Cell::new(true),
        }
    }
    fn trust(&self) -> CheckedTrust<'_> {
        CheckedTrust::new(self, &self.boundaries)
    }
    fn plan(&self, path: &str) -> FilePlan {
        FilePlan::preview(&self.root, [FileInput::new(path, b"bytes")], &self.trust()).unwrap()
    }
}
impl WorkspaceTrust for Fixture {
    fn containing_root(&self, path: &Path) -> Option<PathBuf> {
        (self.approved.get() && path.starts_with(&self.root)).then(|| self.root.clone())
    }
}

#[test]
fn outside_apply_refuses_before_journal_or_writer_effects() {
    let fixture = Fixture::new();
    let plan = fixture.plan("new");
    fixture.approved.set(false);
    assert!(
        apply(&fixture.root, &plan, &fixture.trust()).is_err(),
        "unapproved root wrote files"
    );
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
}

#[test]
fn secret_preview_refuses_before_reading_existing_content() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join(".env"), b"unchanged").unwrap();
    let error = FilePlan::preview(
        &fixture.root,
        [FileInput::new(".env", b"unchanged")],
        &fixture.trust(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("secret"), "{error}");
    assert_eq!(fs::read(fixture.root.join(".env")).unwrap(), b"unchanged");
}

#[test]
fn revocation_denies_remove_and_recovery_without_effects() {
    let fixture = Fixture::new();
    let plan = fixture.plan("owned");
    apply(&fixture.root, &plan, &fixture.trust()).unwrap();
    let before = fs::read(fixture.root.join("owned")).unwrap();
    fixture.approved.set(false);
    assert!(
        remove(&fixture.root, plan.id(), &fixture.trust()).is_err(),
        "revoked removal succeeded"
    );
    assert!(recover(&fixture.root, plan.id(), &fixture.trust()).is_err());
    assert_eq!(fs::read(fixture.root.join("owned")).unwrap(), before);
    assert_eq!(
        fs::read_dir(fixture.root.join(".maestro-files"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn kernel_writes_keep_kernel_authority_without_agent_grants() {
    let fixture = Fixture::new();
    let kernel = fixture.scratch.path.join("kernel");
    fs::write(kernel.join("kernel-record"), b"kernel-owned").unwrap();
    assert!(
        fixture
            .trust()
            .authorize(&kernel, Path::new("kernel-record"), Access::Read)
            .is_err()
    );
    assert!(
        fixture
            .trust()
            .authorize_create(&kernel, Path::new("agent-record"))
            .is_err()
    );
    assert_eq!(
        fs::read(kernel.join("kernel-record")).unwrap(),
        b"kernel-owned"
    );
    assert!(!kernel.join("agent-record").exists());
}

/// Native symlink/junction adapter; the target may be a not-yet-created directory.
fn directory_link(target: &Path, link: &Path) {
    #[cfg(unix)]
    symlink(target, link).unwrap();
    #[cfg(windows)]
    assert!(
        Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(link)
            .arg(target)
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn secret_rebind_denies_apply_remove_and_interrupted_recovery_without_state_writes() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.root.join("keys")).unwrap();
    let plan = fixture.plan("keys/owned");
    apply_with_failure(&fixture.root, &plan, Some(0), &fixture.trust()).unwrap_err();
    let state = fixture.root.join(".maestro-files");
    let before: Vec<_> = fs::read_dir(&state)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect();
    directory_link(
        &fixture.root.join("keys"),
        &fixture.scratch.path.join(".ssh"),
    );
    assert!(apply(&fixture.root, &plan, &fixture.trust()).is_err());
    assert!(recover(&fixture.root, plan.id(), &fixture.trust()).is_err());
    assert!(!fixture.root.join("keys/owned").exists());
    for (name, bytes) in before {
        assert_eq!(fs::read(state.join(name)).unwrap(), bytes);
    }
    #[cfg(unix)]
    fs::remove_file(fixture.scratch.path.join(".ssh")).unwrap();
    #[cfg(windows)]
    fs::remove_dir(fixture.scratch.path.join(".ssh")).unwrap();
    recover(&fixture.root, plan.id(), &fixture.trust()).unwrap();
    let records: Vec<_> = fs::read_dir(&state)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect();
    directory_link(
        &fixture.root.join("keys"),
        &fixture.scratch.path.join(".ssh"),
    );
    assert!(remove(&fixture.root, plan.id(), &fixture.trust()).is_err());
    assert_eq!(fs::read(fixture.root.join("keys/owned")).unwrap(), b"bytes");
    for (name, bytes) in records {
        assert_eq!(fs::read(state.join(name)).unwrap(), bytes);
    }
}

#[test]
fn declined_config_capability_is_exact_and_never_grants_siblings_or_trust() {
    use crate::policy::workspace::{JournalTrust, preferences_confirmation, write_preferences};
    use maestro_kernel::store::Database;
    let fixture = Fixture::new();
    fixture.approved.set(false);
    let data = fixture.scratch.path.join("kernel");
    let database = Database::open_in(&data).unwrap();
    let adapter = JournalTrust::new(&database);
    let trust = CheckedTrust::new(&adapter, &fixture.boundaries);
    let permit = || {
        preferences_confirmation(
            &fixture.root,
            Some(&fixture.root),
            false,
            "",
            (&mut &b""[..], &mut Vec::new()),
        )
        .unwrap()
        .unwrap()
    };
    for target in [
        "sibling",
        ".maestro/other.toml",
        "../config.toml",
        ".maestro/../config.toml",
    ] {
        assert!(
            write_preferences(
                &database,
                &trust,
                &FileInput::new(target, b"bytes"),
                permit()
            )
            .is_err()
        );
        assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
    }
    assert!(database.workspace_answers().unwrap().is_empty());
    let file = FileInput::new(".maestro/config.toml", b"preferences");
    write_preferences(&database, &trust, &file, permit()).unwrap();
    assert_eq!(fs::read(fixture.root.join(&file.path)).unwrap(), file.bytes);
    assert!(database.trusted_workspaces().unwrap().is_empty());
    assert!(
        trust
            .authorize_create(&fixture.root, Path::new("sibling"))
            .is_err()
    );
    assert!(
        trust
            .authorize_create(&data, Path::new("agent-record"))
            .is_err()
    );
    drop(database);
}

#[test]
fn declined_config_capability_refuses_symlinked_parent_with_intact_target() {
    use crate::policy::workspace::{JournalTrust, preferences_confirmation, write_preferences};
    use maestro_kernel::store::Database;
    let fixture = Fixture::new();
    let target = fixture.scratch.path.join("outside");
    fs::create_dir(&target).unwrap();
    directory_link(&target, &fixture.root.join(".maestro"));
    let database = Database::open_in(&fixture.scratch.path.join("kernel")).unwrap();
    let adapter = JournalTrust::new(&database);
    let trust = CheckedTrust::new(&adapter, &fixture.boundaries);
    let permit = preferences_confirmation(
        &fixture.root,
        Some(&fixture.root),
        false,
        "",
        (&mut &b""[..], &mut Vec::new()),
    )
    .unwrap()
    .unwrap();
    let file = FileInput::new(".maestro/config.toml", b"preferences");
    assert!(write_preferences(&database, &trust, &file, permit).is_err());
    assert_eq!(fs::read_dir(target).unwrap().count(), 0);
    assert!(database.trusted_workspaces().unwrap().is_empty());
    drop(database);
}

#[path = "workspace_trust_effects.rs"]
mod effects;
