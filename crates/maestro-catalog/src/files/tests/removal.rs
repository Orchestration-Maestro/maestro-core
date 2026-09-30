use super::support::Scratch;
use crate::files::{FileInput, FilePlan, apply, remove};
use maestro_filesystem::Directory;
use std::{fs, path::Path};

#[test]
fn edited_owned_file_refuses_removal_before_any_owned_bytes_are_removed() {
    let scratch = Scratch::new();
    let plan = FilePlan::preview(
        &scratch.path,
        [
            FileInput::new("first", b"one".to_vec()),
            FileInput::new("second", b"two".to_vec()),
        ],
    )
    .unwrap();
    apply(&scratch.path, &plan).unwrap();
    fs::write(scratch.path.join("second"), b"user edit").unwrap();
    assert!(remove(&scratch.path, plan.id()).is_err());
    assert_eq!(fs::read(scratch.path.join("first")).unwrap(), b"one");
    assert_eq!(fs::read(scratch.path.join("second")).unwrap(), b"user edit");
}

#[test]
fn removing_an_owned_hard_link_name_keeps_the_other_name() {
    let scratch = Scratch::new();
    let plan =
        FilePlan::preview(&scratch.path, [FileInput::new("owned", b"bytes".to_vec())]).unwrap();
    apply(&scratch.path, &plan).unwrap();
    fs::hard_link(scratch.path.join("owned"), scratch.path.join("external")).unwrap();
    remove(&scratch.path, plan.id()).unwrap();
    assert!(!scratch.path.join("owned").exists());
    assert_eq!(fs::read(scratch.path.join("external")).unwrap(), b"bytes");
}

#[test]
fn interrupted_removal_resumes_from_the_committed_ownership_record() {
    let scratch = Scratch::new();
    let plan = FilePlan::preview(
        &scratch.path,
        [
            FileInput::new("first", b"one".to_vec()),
            FileInput::new("second", b"two".to_vec()),
        ],
    )
    .unwrap();
    apply(&scratch.path, &plan).unwrap();
    let directory = Directory::open(&scratch.path, Path::new(""), false).unwrap();
    directory.remove_verified("first", b"one").unwrap();
    remove(&scratch.path, plan.id()).unwrap();
    assert!(!scratch.path.join("first").exists());
    assert!(!scratch.path.join("second").exists());
}
