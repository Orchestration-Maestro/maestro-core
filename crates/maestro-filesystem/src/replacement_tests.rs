//! Atomic replacement contracts; all fixtures contain an unrelated neighbour.
use crate::Directory;
use maestro_test_scratch::scratch_directory;
use std::{fs, io, path::Path};

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
