//! Created-object identity is shared by publication, writes and verified rollback.
use crate::{Directory, PublicationChecks};
use maestro_test_scratch::scratch_directory;
#[cfg(unix)]
use std::os::unix::fs::symlink;
#[cfg(windows)]
use std::process::Command;
use std::{fs, path::Path};

#[test]
fn created_identity_matches_only_regular_same_objects_never_a_symlink_alias() {
    let root = scratch_directory().unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let file = directory.create_new("created").unwrap();
    fs::write(root.join("other"), b"other").unwrap();
    directory.verify_created("created", &file).unwrap();
    assert!(directory.verify_created("other", &file).is_err());
    fs::hard_link(root.join("created"), root.join("hard-link")).unwrap();
    directory.verify_created("hard-link", &file).unwrap();
    #[cfg(unix)]
    symlink(root.join("created"), root.join("symlink")).unwrap();
    #[cfg(windows)]
    assert!(
        Command::new("cmd")
            .args(["/C", "mklink"])
            .arg(root.join("symlink"))
            .arg(root.join("created"))
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(directory.verify_created("symlink", &file).is_err());
    drop(file);
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn publication_source_must_be_regular_with_no_effects_for_a_directory() {
    let root = scratch_directory().unwrap();
    fs::create_dir(root.join("source-directory")).unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let error = directory
        .publish_verified(
            "source-directory",
            "target",
            b"bytes",
            PublicationChecks {
                after_source_open: || Ok(()),
                before_link: || Ok(()),
                after_link: || Ok(()),
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("regular"), "{error}");
    assert!(!root.join("target").exists());
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn publication_checkpoints_run_in_named_open_before_after_order() {
    use std::cell::RefCell;
    let root = scratch_directory().unwrap();
    fs::write(root.join("source"), b"bytes").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let events = RefCell::new(Vec::new());
    directory
        .publish_verified(
            "source",
            "target",
            b"bytes",
            PublicationChecks {
                after_source_open: || {
                    events.borrow_mut().push("open");
                    Ok(())
                },
                before_link: || {
                    events.borrow_mut().push("before");
                    Ok(())
                },
                after_link: || {
                    events.borrow_mut().push("after");
                    Ok(())
                },
            },
        )
        .unwrap();
    assert_eq!(*events.borrow(), ["open", "before", "after"]);
    assert_eq!(fs::read(root.join("target")).unwrap(), b"bytes");
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn publication_identity_is_rechecked_after_source_open_before_read_checkpoint() {
    use std::cell::Cell;
    let root = scratch_directory().unwrap();
    fs::write(root.join("source"), b"bytes").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let later_calls = Cell::new(0);
    let error = directory
        .publish_verified(
            "source",
            "target",
            b"bytes",
            PublicationChecks {
                after_source_open: || {
                    fs::rename(root.join("source"), root.join("moved")).unwrap();
                    fs::write(root.join("source"), b"replacement").unwrap();
                    Ok(())
                },
                before_link: || {
                    later_calls.set(later_calls.get() + 1);
                    Ok(())
                },
                after_link: || {
                    later_calls.set(later_calls.get() + 1);
                    Ok(())
                },
            },
        )
        .unwrap_err();
    assert!(error.to_string().contains("created object changed"));
    assert_eq!(
        later_calls.get(),
        0,
        "source drift must stop before the read and later checkpoints"
    );
    assert!(!root.join("target").exists());
    assert_eq!(fs::read(root.join("source")).unwrap(), b"replacement");
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn publication_post_link_compare_refuses_source_swap_in_the_last_syscall_window() {
    let root = scratch_directory().unwrap();
    fs::write(root.join("source"), b"original").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let error = directory
        .publish_after_compare_for_test("source", "target", b"original", || {
            fs::rename(root.join("source"), root.join("moved")).unwrap();
            fs::write(root.join("source"), b"replacement").unwrap();
        })
        .unwrap_err();
    assert!(error.to_string().contains("rollback failed"));
    assert_eq!(fs::read(root.join("moved")).unwrap(), b"original");
    assert_eq!(fs::read(root.join("source")).unwrap(), b"replacement");
    assert_eq!(
        fs::read(root.join("target")).unwrap(),
        b"replacement",
        "replacement identity must survive refusal"
    );
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn publication_refuses_bytes_different_from_the_completed_write_before_link() {
    use std::cell::Cell;
    let root = scratch_directory().unwrap();
    fs::write(root.join("source"), b"replacement").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let links = Cell::new(0);
    let error = directory
        .publish_verified(
            "source",
            "target",
            b"written bytes",
            PublicationChecks {
                after_source_open: || Ok(()),
                before_link: || {
                    links.set(links.get() + 1);
                    Ok(())
                },
                after_link: || Ok(()),
            },
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("source bytes changed"),
        "{error}"
    );
    assert_eq!(links.get(), 0);
    assert!(!root.join("target").exists());
    assert_eq!(fs::read(root.join("source")).unwrap(), b"replacement");
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn hardened_child_compares_the_returned_handle_even_when_the_name_is_restored() {
    let root = scratch_directory().unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let created = directory.create_child("created").unwrap();
    fs::rename(root.join("created"), root.join("original")).unwrap();
    fs::create_dir(root.join("created")).unwrap();
    let error = directory
        .harden_created_child_with("created", &created, || {
            fs::rename(root.join("created"), root.join("replacement")).unwrap();
            fs::rename(root.join("original"), root.join("created")).unwrap();
        })
        .unwrap_err();
    assert!(
        error.to_string().contains("created directory changed"),
        "{error}"
    );
    drop(created);
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn verified_removal_restores_and_propagates_the_policy_error() {
    use std::io::{self, ErrorKind};
    let root = scratch_directory().unwrap();
    fs::write(root.join("owned"), b"bytes").unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let error = directory
        .remove_verified_checked("owned", b"bytes", None, || {
            Err(io::Error::new(
                ErrorKind::PermissionDenied,
                "policy revoked",
            ))
        })
        .unwrap_err();
    assert_eq!(error.kind(), ErrorKind::PermissionDenied);
    assert_eq!(error.to_string(), "policy revoked");
    assert_eq!(fs::read(root.join("owned")).unwrap(), b"bytes");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn created_directory_native_rollback_deletes_only_empty_original_identity() {
    let root = scratch_directory().unwrap();
    let directory = Directory::open(&root, Path::new(""), false).unwrap();
    let created = directory.create_child("created").unwrap();
    fs::write(root.join("created/child"), b"user").unwrap();
    assert!(directory.remove_created_child("created", &created).is_err());
    assert_eq!(fs::read(root.join("created/child")).unwrap(), b"user");
    fs::remove_file(root.join("created/child")).unwrap();
    fs::rename(root.join("created"), root.join("moved")).unwrap();
    fs::create_dir(root.join("created")).unwrap();
    let error = directory
        .remove_created_child("created", &created)
        .unwrap_err();
    assert!(error.to_string().contains("created directory changed"));
    assert!(root.join("created").is_dir());
    assert!(root.join("moved").is_dir());
    fs::remove_dir(root.join("created")).unwrap();
    fs::rename(root.join("moved"), root.join("created")).unwrap();
    directory.remove_created_child("created", &created).unwrap();
    drop(created);
    assert!(!root.join("created").exists());
    drop(directory);
    fs::remove_dir_all(root).unwrap();
}
