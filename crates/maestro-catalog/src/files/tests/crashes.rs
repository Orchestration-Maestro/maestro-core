use super::super::apply::apply_with_failure;
use super::support::Scratch;
use crate::files::{FileInput, FilePlan, apply, recover, remove};
use std::fs;

#[test]
fn every_interrupted_write_or_ownership_commit_recovers_idempotently() {
    for point in 0..=6 {
        let scratch = Scratch::new();
        let plan = FilePlan::preview(
            &scratch.path,
            [
                FileInput::new("nested/one", b"first".to_vec()),
                FileInput::new("two", b"second".to_vec()),
            ],
        )
        .unwrap();
        assert!(apply_with_failure(&scratch.path, &plan, Some(point)).is_err());
        if point == 0 || point == 1 || point == 6 {
            recover(&scratch.path, plan.id()).unwrap();
            assert_eq!(fs::read(scratch.path.join("nested/one")).unwrap(), b"first");
            assert_eq!(fs::read(scratch.path.join("two")).unwrap(), b"second");
            remove(&scratch.path, plan.id()).unwrap();
            assert!(!scratch.path.join("nested/one").exists());
            assert!(!scratch.path.join("two").exists());
        } else {
            assert!(recover(&scratch.path, plan.id()).is_err());
            assert!(fs::read(scratch.path.join("nested/one")).is_ok());
            assert!(
                !scratch
                    .path
                    .join(".maestro-files")
                    .join(format!("ownership-{}.toml", plan.id()))
                    .exists()
            );
        }
    }
}

#[test]
fn a_stale_preview_never_overwrites_a_new_file() {
    let scratch = Scratch::new();
    let plan = FilePlan::preview(
        &scratch.path,
        [FileInput::new("target", b"planned".to_vec())],
    )
    .unwrap();
    fs::write(scratch.path.join("target"), b"planned").unwrap();
    assert!(apply(&scratch.path, &plan).is_err());
    assert_eq!(fs::read(scratch.path.join("target")).unwrap(), b"planned");
    assert!(remove(&scratch.path, plan.id()).is_err());
    assert_eq!(fs::read(scratch.path.join("target")).unwrap(), b"planned");
}

#[test]
fn recovery_refuses_an_identical_user_file_without_committing_ownership() {
    let scratch = Scratch::new();
    let plan = FilePlan::preview(
        &scratch.path,
        [FileInput::new("target", b"planned".to_vec())],
    )
    .unwrap();
    assert!(apply_with_failure(&scratch.path, &plan, Some(1)).is_err());
    fs::write(scratch.path.join("target"), b"planned").unwrap();
    assert!(recover(&scratch.path, plan.id()).is_err());
    assert_eq!(fs::read(scratch.path.join("target")).unwrap(), b"planned");
    assert!(remove(&scratch.path, plan.id()).is_err());
}

#[test]
fn traversal_and_reserved_paths_are_rejected_before_a_file_is_created() {
    let scratch = Scratch::new();
    for path in ["../outside", "a/../outside", ".maestro-files/journal"] {
        assert!(FilePlan::preview(&scratch.path, [FileInput::new(path, b"no".to_vec())]).is_err());
    }
    assert!(
        FilePlan::preview(
            &scratch.path,
            [
                FileInput::new("node", b"file".to_vec()),
                FileInput::new("node/child", b"child".to_vec())
            ]
        )
        .is_err()
    );
    assert!(recover(&scratch.path, "../../outside").is_err());
    assert!(remove(&scratch.path, "../../outside").is_err());
    assert!(!scratch.path.join(".maestro-files").exists());
}
