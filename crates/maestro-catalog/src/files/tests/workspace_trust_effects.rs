//! Scheduling proofs for the shared owned-effect port and native rollback identities.
use super::{Access, Cell, Fixture, Path, apply, directory_link, fs, recover, remove};

#[test]
fn new_parents_and_written_bytes_roll_back_on_post_effect_revocation() {
    let fixture = Fixture::new();
    let directory = fixture
        .trust()
        .authorize(&fixture.root, Path::new("new-parent"), Access::Write)
        .unwrap();
    assert!(
        directory
            .create_directory_for_test(|| fixture.approved.set(false))
            .is_err()
    );
    assert!(!fixture.root.join("new-parent").exists());
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 0);
    fixture.approved.set(true);
    let write = fixture
        .trust()
        .authorize_create(&fixture.root, Path::new(".maestro/config.toml"))
        .unwrap();
    assert!(
        write
            .write_new_with(b"bytes", || fixture.approved.set(false))
            .is_err()
    );
    assert!(!fixture.root.join(".maestro/config.toml").exists());
    assert_eq!(
        fs::read_dir(fixture.root.join(".maestro")).unwrap().count(),
        0
    );
}

#[test]
fn new_effects_recheck_a_retained_capability_before_writing_or_removing() {
    let fixture = Fixture::new();
    let write = fixture
        .trust()
        .authorize(&fixture.root, Path::new("new"), Access::Write)
        .unwrap();
    let directory = fixture
        .trust()
        .authorize(&fixture.root, Path::new("new-parent"), Access::Write)
        .unwrap();
    fs::write(fixture.root.join("owned"), b"bytes").unwrap();
    let remove = fixture
        .trust()
        .authorize(&fixture.root, Path::new("owned"), Access::Write)
        .unwrap();
    fixture.approved.set(false);
    let writer_calls = Cell::new(0);
    let effect = || writer_calls.set(writer_calls.get() + 1);
    assert!(write.write_new_with(b"bytes", effect).is_err());
    assert!(directory.create_directory_for_test(effect).is_err());
    assert!(remove.remove_verified_with(b"bytes", None, effect).is_err());
    assert_eq!(
        writer_calls.get(),
        0,
        "denial must precede every writer call"
    );
    assert_eq!(fs::read(fixture.root.join("owned")).unwrap(), b"bytes");
    assert!(!fixture.root.join("new").exists());
    assert!(!fixture.root.join("new-parent").exists());
}

#[test]
fn deny_rebind_between_check_and_mkdir_reaches_native_directory_rollback() {
    let fixture = Fixture::new();
    let target = fixture.root.join("new-parent");
    let directory = fixture
        .trust()
        .authorize(&fixture.root, Path::new("new-parent"), Access::Write)
        .unwrap();
    let error = directory
        .create_directory_between_for_test(|| {
            directory_link(&target, &fixture.scratch.path.join(".ssh"));
        })
        .unwrap_err();
    assert!(error.to_string().contains("secret"), "{error}");
    assert!(
        !target.exists(),
        "denied mkdir must be rolled back on every native host"
    );
    #[cfg(unix)]
    fs::remove_file(fixture.scratch.path.join(".ssh")).unwrap();
    #[cfg(windows)]
    fs::remove_dir(fixture.scratch.path.join(".ssh")).unwrap();
    assert!(
        fixture
            .trust()
            .authorize_create(&fixture.root, Path::new("neighbour/plain"))
            .unwrap()
            .write_new(b"allowed")
            .is_ok()
    );
}

#[test]
fn nonempty_directory_rollback_refuses_and_retains_other_process_bytes() {
    let fixture = Fixture::new();
    let directory = fixture
        .trust()
        .authorize(&fixture.root, Path::new("new-parent"), Access::Write)
        .unwrap();
    let error = directory
        .create_directory_for_test(|| {
            fs::write(fixture.root.join("new-parent/other"), b"other process").unwrap();
            fixture.approved.set(false);
        })
        .unwrap_err();
    assert!(error.to_string().contains("rollback failed"), "{error}");
    assert_eq!(
        fs::read(fixture.root.join("new-parent/other")).unwrap(),
        b"other process"
    );
}

#[cfg(unix)]
#[test]
fn directory_rollback_restores_a_replacement_instead_of_deleting_it() {
    let fixture = Fixture::new();
    let directory = fixture
        .trust()
        .authorize(&fixture.root, Path::new("new-parent"), Access::Write)
        .unwrap();
    let error = directory
        .create_directory_for_test(|| {
            fs::rename(
                fixture.root.join("new-parent"),
                fixture.root.join("created-moved"),
            )
            .unwrap();
            fs::create_dir(fixture.root.join("new-parent")).unwrap();
            fixture.approved.set(false);
        })
        .unwrap_err();
    assert!(error.to_string().contains("rollback failed"), "{error}");
    assert!(
        fixture.root.join("new-parent").is_dir(),
        "replacement must be restored"
    );
    assert!(fixture.root.join("created-moved").is_dir());
}

#[test]
fn removal_restores_quarantined_bytes_when_post_effect_policy_refuses() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("owned"), b"bytes").unwrap();
    let remove = fixture
        .trust()
        .authorize(&fixture.root, Path::new("owned"), Access::Write)
        .unwrap();
    assert!(
        remove
            .remove_verified_with(b"bytes", None, || fixture.approved.set(false))
            .is_err()
    );
    assert_eq!(fs::read(fixture.root.join("owned")).unwrap(), b"bytes");
    assert_eq!(fs::read_dir(&fixture.root).unwrap().count(), 1);
}

#[test]
fn written_file_rollback_preserves_a_replacement_with_identical_bytes() {
    let fixture = Fixture::new();
    let write = fixture
        .trust()
        .authorize(&fixture.root, Path::new("new"), Access::Write)
        .unwrap();
    let error = write
        .write_new_with(b"bytes", || {
            fs::rename(fixture.root.join("new"), fixture.root.join("created-moved")).unwrap();
            fs::write(fixture.root.join("new"), b"bytes").unwrap();
            fixture.approved.set(false);
        })
        .unwrap_err();
    assert!(error.to_string().contains("rollback failed"), "{error}");
    assert_eq!(fs::read(fixture.root.join("new")).unwrap(), b"bytes");
}

#[test]
fn swapped_parents_deny_apply_remove_and_recovery_with_intact_owned_bytes() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.root.join("parent")).unwrap();
    let plan = fixture.plan("parent/owned");
    apply(&fixture.root, &plan, &fixture.trust()).unwrap();
    let saved = fixture.scratch.path.join("saved");
    fs::rename(fixture.root.join("parent"), &saved).unwrap();
    directory_link(&saved, &fixture.root.join("parent"));
    let before = fs::read(saved.join("owned")).unwrap();
    let records_before: Vec<_> = fs::read_dir(fixture.root.join(".maestro-files"))
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name(), fs::read(entry.path()).unwrap())
        })
        .collect();
    assert!(apply(&fixture.root, &plan, &fixture.trust()).is_err());
    assert!(remove(&fixture.root, plan.id(), &fixture.trust()).is_err());
    assert!(recover(&fixture.root, plan.id(), &fixture.trust()).is_err());
    assert_eq!(fs::read(saved.join("owned")).unwrap(), before);
    for (name, bytes) in records_before {
        assert_eq!(
            fs::read(fixture.root.join(".maestro-files").join(name)).unwrap(),
            bytes
        );
    }
}

#[test]
fn directory_hardening_refuses_replacement_before_returning_a_parent_grant() {
    let fixture = Fixture::new();
    let directory = fixture
        .trust()
        .authorize(&fixture.root, Path::new("new-parent"), Access::Write)
        .unwrap();
    let error = directory
        .create_directory_for_test(|| {
            fs::rename(
                fixture.root.join("new-parent"),
                fixture.root.join("created-moved"),
            )
            .unwrap();
            fs::create_dir(fixture.root.join("new-parent")).unwrap();
        })
        .unwrap_err();
    assert!(
        error.to_string().contains("created object changed"),
        "{error}"
    );
    assert!(error.to_string().contains("rollback failed"), "{error}");
    assert!(fixture.root.join("new-parent").is_dir());
    assert!(fixture.root.join("created-moved").is_dir());
}

#[cfg(windows)]
#[test]
fn new_directory_is_hardened_against_rename_before_later_file_effects() {
    let fixture = Fixture::new();
    let write = fixture
        .trust()
        .authorize_create(&fixture.root, Path::new("new-parent/new"))
        .unwrap();
    assert!(fs::rename(fixture.root.join("new-parent"), fixture.root.join("moved")).is_err());
    write.write_new(b"bytes").unwrap();
    assert_eq!(
        fs::read(fixture.root.join("new-parent/new")).unwrap(),
        b"bytes"
    );
}

#[test]
fn written_bytes_are_not_owned_when_the_created_name_is_replaced() {
    let fixture = Fixture::new();
    let write = fixture
        .trust()
        .authorize(&fixture.root, Path::new("new"), Access::Write)
        .unwrap();
    let result = write.write_new_with(b"bytes", || {
        fs::rename(fixture.root.join("new"), fixture.root.join("created-moved")).unwrap();
        fs::write(fixture.root.join("new"), b"replacement").unwrap();
    });
    assert!(
        result.is_err(),
        "a replacement must not gain ownership metadata"
    );
    assert_eq!(fs::read(fixture.root.join("new")).unwrap(), b"replacement");
    assert_eq!(
        fs::read(fixture.root.join("created-moved")).unwrap(),
        b"bytes"
    );
}

#[test]
fn written_file_rollback_preserves_edits_to_the_created_object() {
    let fixture = Fixture::new();
    let write = fixture
        .trust()
        .authorize(&fixture.root, Path::new("new"), Access::Write)
        .unwrap();
    let error = write
        .write_new_with(b"ready", || {
            fs::write(fixture.root.join("new"), b"edits").unwrap();
            fixture.approved.set(false);
        })
        .unwrap_err();
    assert!(error.to_string().contains("rollback failed"));
    assert_eq!(fs::read(fixture.root.join("new")).unwrap(), b"edits");
}
