//! Windows full-volume/full-file identities from real held files.
use crate::windows_security::{file_identity_with, same_file};
use maestro_test_scratch::scratch_directory;
use std::{
    fs::{self, File},
    io,
};

#[test]
fn two_handles_to_one_file_have_equal_full_identity() {
    let root = scratch_directory().unwrap();
    fs::write(root.join("plain"), b"same").unwrap();
    let left = File::open(root.join("plain")).unwrap();
    let right = File::open(root.join("plain")).unwrap();
    assert!(same_file(&left, &right).unwrap());
    drop((left, right));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn different_and_renamed_replaced_files_have_unequal_identity() {
    let root = scratch_directory().unwrap();
    fs::write(root.join("plain"), b"old").unwrap();
    fs::write(root.join("other"), b"other").unwrap();
    let held = File::open(root.join("plain")).unwrap();
    let other = File::open(root.join("other")).unwrap();
    assert!(!same_file(&held, &other).unwrap());
    fs::rename(root.join("plain"), root.join("moved")).unwrap();
    fs::write(root.join("plain"), b"replacement").unwrap();
    let replaced = File::open(root.join("plain")).unwrap();
    let moved = File::open(root.join("moved")).unwrap();
    assert!(!same_file(&held, &replaced).unwrap());
    assert!(same_file(&held, &moved).unwrap());
    drop((held, other, replaced, moved));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn injected_identity_api_failure_refuses_without_a_fallback() {
    let root = scratch_directory().unwrap();
    fs::write(root.join("plain"), b"same").unwrap();
    let file = File::open(root.join("plain")).unwrap();
    assert!(file_identity_with(&file, |_, _| Ok(0)).is_err());
    assert!(
        file_identity_with(&file, |_, _| Err(io::Error::other(
            "injected identity failure"
        )))
        .is_err()
    );
    assert!(same_file(&file, &file).unwrap());
    drop(file);
    fs::remove_dir_all(root).unwrap();
}
