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
fn replacement_retries_only_existing_temporary_names() {
    fixture(|root, directory| {
        for sequence in 0..100 {
            fs::write(
                root.join(format!(".maestro-replace-{}-{sequence}", process::id())),
                b"user",
            )
            .unwrap();
        }
        directory
            .replace_verified("target", b"old", b"new", || Ok(()))
            .unwrap();
        assert_eq!(fs::read(root.join("target")).unwrap(), b"new");
        for sequence in 0..100 {
            assert_eq!(
                fs::read(root.join(format!(".maestro-replace-{}-{sequence}", process::id())))
                    .unwrap(),
                b"user"
            );
        }
    });
}

#[cfg(unix)]
#[test]
fn replacement_temporary_creation_error_is_not_retried() {
    use std::os::unix::fs::PermissionsExt as _;
    fixture(|root, directory| {
        fs::set_permissions(root, fs::Permissions::from_mode(0o500)).unwrap();
        let result = directory.replace_verified("target", b"old", b"new", || Ok(()));
        fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(fs::read(root.join("target")).unwrap(), b"old");
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
    use crate::windows_security::DisabledAclBypass;
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
            r#"param([string]$Root, [switch]$SnapshotOnly)
$ErrorActionPreference = 'Stop'
$leafPath = Join-Path $Root 'target'
$leaf = Get-Acl -LiteralPath $leafPath
$original = [Convert]::ToBase64String($leaf.GetSecurityDescriptorBinaryForm())
$descriptor = [System.Security.AccessControl.RawSecurityDescriptor]::new(
    $leaf.GetSecurityDescriptorBinaryForm(), 0)
$bytes = [byte[]]::new($descriptor.DiscretionaryAcl.BinaryLength)
$descriptor.DiscretionaryAcl.GetBinaryForm($bytes, 0)
$snapshot = '{0}|{1}|{2}' -f $descriptor.Owner.Value,
    [Convert]::ToBase64String($bytes), $leaf.AreAccessRulesProtected
if ($SnapshotOnly) { $snapshot; exit }
if ($leaf.AreAccessRulesProtected) { throw 'fixture leaf must be unprotected' }
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class NativeAcl {
    [DllImport("advapi32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern bool SetFileSecurity(string path, uint information, byte[] descriptor);
}
'@
$acl = Get-Acl -LiteralPath $Root
$sid = [System.Security.Principal.SecurityIdentifier]::new('S-1-1-0')
$rule = [System.Security.AccessControl.FileSystemAccessRule]::new(
    $sid, 'ReadAndExecute', 'ContainerInherit,ObjectInherit', 'None', 'Allow')
$acl.AddAccessRule($rule)
# SetFileSecurity updates this directory only, not its existing children.
if (-not [NativeAcl]::SetFileSecurity($Root, 4, $acl.GetSecurityDescriptorBinaryForm())) {
    throw [System.ComponentModel.Win32Exception]::new(
        [Runtime.InteropServices.Marshal]::GetLastWin32Error())
}
$sections = [System.Security.AccessControl.AccessControlSections]::Access
$parent = Get-Acl -LiteralPath $Root
if ($parent.GetSecurityDescriptorSddlForm($sections) -eq
    $leaf.GetSecurityDescriptorSddlForm($sections)) {
    throw 'parent DACL must differ from the leaf DACL'
}
if ([Convert]::ToBase64String((Get-Acl -LiteralPath $leafPath).
    GetSecurityDescriptorBinaryForm()) -ne $original) {
    throw 'broadening the parent must not change the leaf security'
}
$snapshot
"#,
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
