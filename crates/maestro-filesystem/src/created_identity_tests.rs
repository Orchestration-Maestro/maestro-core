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
        .publish_after_compare_for_test("source", "target", || {
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
