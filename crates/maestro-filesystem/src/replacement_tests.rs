//! Atomic replacement contracts; all fixtures contain an unrelated neighbour.
use crate::Directory;
use maestro_test_scratch::scratch_directory;
use std::{cell::RefCell, fs, io, path::Path, process};

fn fixture(run: impl FnOnce(&Path, &Directory)) {
    let root = scratch_directory().unwrap();
    fs::write(root.join("target"), b"old").unwrap();
    fs::write(root.join("neighbour"), b"user").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    run(&root, &directory);
    assert_eq!(fs::read(root.join("neighbour")).unwrap(), b"user");
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replacement_preserves_neighbour_and_permissions() {
    fixture(|root, directory| {
        let permissions = fs::metadata(root.join("target")).unwrap().permissions();
        directory
            .replace_verified("target", b"old", b"new", || Ok(()))
            .unwrap();
        assert_eq!(fs::read(root.join("target")).unwrap(), b"new");
        assert_eq!(
            fs::metadata(root.join("target")).unwrap().permissions(),
            permissions
        );
    });
}

#[test]
fn replacement_refuses_drift_and_interrupts_before_atomic_swap() {
    fixture(|root, directory| {
        assert!(
            directory
                .replace_verified("target", b"different", b"new", || Ok(()))
                .is_err()
        );
        assert!(
            directory
                .replace_verified("target", b"old", b"new", || Err(io::Error::other("crash")))
                .is_err()
        );
        assert_eq!(fs::read(root.join("target")).unwrap(), b"old");
        assert_eq!(fs::read_dir(root).unwrap().count(), 2);
        directory
            .replace_verified("target", b"old", b"new", || Ok(()))
            .unwrap();
        assert_eq!(fs::read(root.join("target")).unwrap(), b"new");
    });
}

#[test]
fn replacement_rechecks_target_after_preparation() {
    fixture(|root, directory| {
        let result = directory.replace_verified("target", b"old", b"new", || {
            fs::write(root.join("target"), b"edited")?;
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(fs::read(root.join("target")).unwrap(), b"edited");
    });
}

#[cfg(unix)]
#[test]
fn replacement_refuses_link_substitution() {
    use std::os::unix::fs::symlink;
    fixture(|root, directory| {
        let result = directory.replace_verified("target", b"old", b"new", || {
            fs::remove_file(root.join("target"))?;
            symlink("neighbour", root.join("target"))
        });
        assert!(result.is_err());
        assert!(
            fs::symlink_metadata(root.join("target"))
                .unwrap()
                .file_type()
                .is_symlink()
        );
    });
}

#[test]
fn replacement_refuses_prepared_sibling_edits() {
    fixture(|root, directory| {
        let mut sibling = None;
        let result = directory.replace_verified("target", b"old", b"new", || {
            let path = fs::read_dir(root)?
                .map(Result::unwrap)
                .find(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".maestro-replace-")
                })
                .unwrap()
                .path();
            fs::write(&path, b"edited")?;
            sibling = Some(path);
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(fs::read(root.join("target")).unwrap(), b"old");
        assert_eq!(fs::read(sibling.unwrap()).unwrap(), b"edited");
    });
}

#[cfg(windows)]
#[test]
fn replacement_preserves_restrictive_windows_dacl() {
    use std::{env, process::Command};
    fixture(|root, directory| {
        let account = env::var("USERNAME").unwrap();
        let run = |path: &Path, args: &[&str]| {
            assert!(
                Command::new("icacls")
                    .arg(path)
                    .args(args)
                    .status()
                    .unwrap()
                    .success()
            );
        };
        run(root, &["/grant", "*S-1-1-0:(OI)(CI)(RX)"]);
        let target = root.join("target");
        run(&target, &["/setowner", &account]);
        run(
            &target,
            &["/inheritance:r", "/grant:r", &format!("{account}:(F)")],
        );
        let before = root.join("before.acl");
        let after = root.join("after.acl");
        run(&target, &["/save", before.to_str().unwrap()]);
        directory
            .replace_verified("target", b"old", b"new", || Ok(()))
            .unwrap();
        run(&target, &["/save", after.to_str().unwrap()]);
        assert_eq!(fs::read(before).unwrap(), fs::read(after).unwrap());
    });
}

#[test]
fn replacement_identity_guard_neighbours() {
    fixture(|root, directory| {
        fs::write(root.join("target"), b"old-extra").unwrap();
        assert!(
            directory
                .replace_verified("target", b"old", b"new", || Ok(()))
                .is_err()
        );
        assert_eq!(fs::read(root.join("target")).unwrap(), b"old-extra");
        fs::write(root.join("target"), b"old").unwrap();
        let mut substituted = None;
        let result = directory.replace_verified("target", b"old", b"new", || {
            let path = fs::read_dir(root)?
                .map(Result::unwrap)
                .find(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".maestro-replace-")
                })
                .unwrap()
                .path();
            fs::rename(&path, root.join("held-stage"))?;
            fs::write(&path, b"new")?;
            substituted = Some(path);
            Ok(())
        });
        assert!(result.is_err());
        assert_eq!(fs::read(root.join("target")).unwrap(), b"old");
        assert_eq!(fs::read(substituted.unwrap()).unwrap(), b"new");
    });
}

#[cfg(unix)]
#[test]
fn replacement_identity_guard_moved_parent() {
    let root = scratch_directory().unwrap();
    let parent = root.join("parent");
    fs::create_dir(&parent).unwrap();
    fs::write(parent.join("target"), b"old").unwrap();
    let directory = Directory::open(&root, Path::new("parent"), false).unwrap();
    let result = directory.replace_verified("target", b"old", b"new", || {
        fs::rename(&parent, root.join("moved"))?;
        fs::create_dir(&parent)?;
        Ok(())
    });
    assert!(result.is_err());
    assert_eq!(fs::read(root.join("moved/target")).unwrap(), b"old");
    assert_eq!(fs::read_dir(&parent).unwrap().count(), 0);
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn replacement_temporary_collision_is_retried() {
    fixture(|root, directory| {
        let name = format!(".maestro-replace-{}-0", process::id());
        fs::write(root.join(&name), b"user").unwrap();
        let mut calls = 0;
        let (created, file) = directory
            .replacement_temp(|| {
                let sequence = calls;
                calls += 1;
                sequence
            })
            .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(created, format!(".maestro-replace-{}-1", process::id()));
        assert_eq!(fs::read(root.join(name)).unwrap(), b"user");
        drop(file);
    });
}

#[test]
fn replacement_temporary_exhausts_named_retry_limit() {
    use crate::replacement::REPLACEMENT_TEMP_ATTEMPTS;
    fixture(|root, directory| {
        fs::write(
            root.join(format!(".maestro-replace-{}-0", process::id())),
            b"user",
        )
        .unwrap();
        let mut calls = 0;
        let error = directory
            .replacement_temp(|| {
                calls += 1;
                0
            })
            .unwrap_err();
        assert_eq!(calls, REPLACEMENT_TEMP_ATTEMPTS);
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(error.to_string().contains(&format!(
            "exhausted {REPLACEMENT_TEMP_ATTEMPTS} replacement temporary retries"
        )));
    });
}

#[cfg(unix)]
#[test]
fn replacement_temporary_permission_error_is_not_retried() {
    use std::os::unix::fs::PermissionsExt as _;
    fixture(|root, directory| {
        fs::set_permissions(root, fs::Permissions::from_mode(0o500)).unwrap();
        let mut calls = 0;
        let result = directory.replacement_temp(|| {
            calls += 1;
            0
        });
        fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(calls, 1);
    });
}

#[test]
fn replacement_post_publication_failure_retains_published_bytes() {
    fixture(|root, directory| {
        let error = directory
            .replace_verified_with(
                "target",
                (b"old", b"new"),
                || Ok(()),
                || Err(io::Error::new(io::ErrorKind::Interrupted, "sync failure")),
            )
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted);
        assert_eq!(fs::read(root.join("target")).unwrap(), b"new");
        assert_eq!(fs::read_dir(root).unwrap().count(), 2);
    });
}

#[test]
fn replacement_post_publication_cleanup_refuses_substituted_temporary() {
    fixture(|root, directory| {
        let temporary = RefCell::new(None);
        let result = directory.replace_verified_with(
            "target",
            (b"old", b"new"),
            || {
                *temporary.borrow_mut() = fs::read_dir(root)?
                    .map(Result::unwrap)
                    .find(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .starts_with(".maestro-replace-")
                    })
                    .map(|entry| entry.path());
                Ok(())
            },
            || {
                fs::create_dir(temporary.borrow().as_ref().unwrap())?;
                Err(io::Error::new(io::ErrorKind::Interrupted, "sync failure"))
            },
        );
        assert_ne!(result.unwrap_err().kind(), io::ErrorKind::Interrupted);
        assert!(temporary.into_inner().unwrap().is_dir());
        assert_eq!(fs::read(root.join("target")).unwrap(), b"new");
    });
}

#[cfg(windows)]
#[test]
fn replacement_refuses_foreign_windows_owner_without_restore_privilege() {
    use crate::windows_test_security::DisabledAclBypass;
    use std::process::Command;
    fixture(|root, directory| {
        let target = root.join("target");
        assert!(
            Command::new("icacls")
                .arg(&target)
                .args(["/setowner", "*S-1-5-18"])
                .status()
                .unwrap()
                .success()
        );
        let privileges = DisabledAclBypass::new().unwrap();
        assert!(privileges.both_disabled().unwrap());
        assert!(
            directory
                .replace_verified("target", b"old", b"new", || Ok(()))
                .is_err()
        );
        assert_eq!(fs::read(target).unwrap(), b"old");
        for entry in fs::read_dir(root).unwrap().map(Result::unwrap) {
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with(".maestro-replace-")
            {
                assert!(
                    fs::read(entry.path()).unwrap().is_empty(),
                    "contents must not be staged under broader security"
                );
            }
        }
    });
}

#[cfg(windows)]
#[test]
fn replacement_retains_or_refuses_windows_security_below_broader_parent() {
    use maestro_test_scratch::disk_scratch_directory;
    use std::process::Command;
    fixture(|root, directory| {
        let target = root.join("target");
        let scripts = disk_scratch_directory().unwrap();
        let script = scripts.join("broaden-parent.ps1");
        fs::write(
            &script,
            include_str!("../tests/fixtures/broaden-parent.ps1"),
        )
        .unwrap();
        let snapshot = |only: bool| {
            let mut command = Command::new("powershell.exe");
            command
                .args(["-NoProfile", "-NonInteractive", "-File"])
                .arg(&script)
                .arg("-Root")
                .arg(root);
            if only {
                command.arg("-SnapshotOnly");
            }
            let output = command.output().unwrap();
            assert!(output.status.success(), "{output:?}");
            assert!(!output.stdout.is_empty());
            output.stdout
        };
        let original_security = snapshot(false);
        match directory.replace_verified("target", b"old", b"new", || Ok(())) {
            Ok(()) => {
                println!("broader parent: exact security retained");
                assert_eq!(fs::read(&target).unwrap(), b"new");
            }
            Err(error) => {
                println!("broader parent: security mismatch refused");
                assert!(
                    error
                        .to_string()
                        .contains("replacement would change the file's access protection")
                );
                assert_eq!(fs::read(&target).unwrap(), b"old");
            }
        }
        // Snapshot compares only the owner's SID, exact DACL bytes and protection control.
        assert_eq!(snapshot(true), original_security);
        assert!(
            fs::read_dir(root)
                .unwrap()
                .map(Result::unwrap)
                .all(|entry| {
                    !entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".maestro-replace-")
                })
        );
        fs::remove_dir_all(scripts).unwrap();
    });
}

#[cfg(windows)]
#[test]
fn replacement_retains_assignable_windows_group_owner() {
    use crate::windows_security_fixture_tests::{icacls, native_owner};
    use std::env;
    fixture(|root, directory| {
        let target = root.join("target");
        let original_owner = native_owner(&target);
        icacls(&target, &["/setowner", "*S-1-5-32-544"]);
        let account = env::var("USERNAME").unwrap();
        icacls(
            &target,
            &["/inheritance:r", "/grant:r", &format!("{account}:(F)")],
        );
        let group = native_owner(&target);
        assert_eq!(group, "S-1-5-32-544");
        assert_ne!(
            group, original_owner,
            "fixture must differ from newly created file owner"
        );
        directory
            .replace_verified("target", b"old", b"new", || Ok(()))
            .unwrap();
        assert_eq!(native_owner(&target), group);
        assert_eq!(fs::read(target).unwrap(), b"new");
    });
}

#[cfg(windows)]
#[test]
fn replacement_temporary_permission_error_is_not_retried() {
    use crate::{windows_security_fixture_tests::icacls, windows_test_security::DisabledAclBypass};
    use std::env;
    fixture(|root, directory| {
        let account = env::var("USERNAME").unwrap();
        icacls(root, &["/deny", &format!("{account}:(WD)")]);
        let privileges = DisabledAclBypass::new().unwrap();
        assert!(privileges.both_disabled().unwrap());
        let mut calls = 0;
        let result = directory.replacement_temp(|| {
            calls += 1;
            0
        });
        icacls(root, &["/remove:d", &account]);
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(calls, 1);
    });
}
