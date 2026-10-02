use super::super::{
    names::{journal_name, ownership_name},
    plan::validate_relative_path,
};
use super::support::Scratch;
use super::support::{apply, apply_with_failure, preview, recover, remove};
use crate::files::FileInput;
use std::fs;

#[test]
fn every_id_derived_state_component_is_windows_safe() {
    for point in [0, 100, 101] {
        let scratch = Scratch::new();
        let plan = preview(
            &scratch.path,
            [FileInput::new("target", b"planned".to_vec())],
        )
        .unwrap();
        let error = apply_with_failure(&scratch.path, &plan, Some(point)).unwrap_err();
        assert!(error.to_string().contains("injected"), "{error}");
        for entry in fs::read_dir(scratch.path.join(".maestro-files")).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            assert!(
                validate_relative_path(&name).is_ok(),
                "ID-derived state component is not Windows-safe: {name}"
            );
            assert!(
                name.bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-')),
                "unexpected state component byte: {name}"
            );
        }
    }
}

#[test]
fn portable_state_names_preserve_distinct_plan_ids_and_record_bytes() {
    let scratch = Scratch::new();
    let inputs = [FileInput::new("target", b"planned".to_vec())];
    let plan = preview(&scratch.path, inputs.clone()).unwrap();
    // Hand-computed SHA-256 of length-prefixed "target" and "planned".
    let id = "sha256:f2f4a7d30e5f1bd26ee8accf0d8be8bfdb80c0523cc245e9b869b895286db70d";
    assert_eq!(plan.id(), id);
    let changed = preview(
        &scratch.path,
        [FileInput::new("target", b"changed".to_vec())],
    )
    .unwrap();
    assert_ne!(journal_name(id), journal_name(changed.id()));
    assert_ne!(ownership_name(id), ownership_name(changed.id()));
    assert_ne!(journal_name(id), ownership_name(id));

    assert!(apply_with_failure(&scratch.path, &plan, Some(0)).is_err());
    let state = scratch.path.join(".maestro-files");
    let journal: toml::Value =
        toml::from_str(&fs::read_to_string(state.join(journal_name(id))).unwrap()).unwrap();
    assert_eq!(journal["id"].as_str(), Some(id));
    assert_eq!(
        journal["entries"][0]["digest"].as_str(),
        Some("sha256:9b0f1b10aff55228716a1fbbc59bda8fe735ed14ccd5e2c5226a9ab72a48d47e")
    );
    recover(&scratch.path, id).unwrap();
    let ownership: toml::Value =
        toml::from_str(&fs::read_to_string(state.join(ownership_name(id))).unwrap()).unwrap();
    assert_eq!(ownership["id"].as_str(), Some(id));
    assert_eq!(
        ownership["files"][0]["digest"],
        journal["entries"][0]["digest"]
    );
    assert!(preview(&scratch.path, inputs).unwrap().is_applied());
    remove(&scratch.path, id).unwrap();
    assert!(!state.join(ownership_name(id)).exists());
    assert!(!scratch.path.join("target").exists());
}

#[test]
fn every_interrupted_write_or_ownership_commit_recovers_idempotently() {
    for point in 0..=6 {
        let scratch = Scratch::new();
        let plan = preview(
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
                    .join(ownership_name(plan.id()))
                    .exists()
            );
        }
    }
}

#[test]
fn interrupted_temporary_records_do_not_block_retry_or_leave_torn_published_files() {
    let scratch = Scratch::new();
    let plan = preview(
        &scratch.path,
        [FileInput::new("target", b"planned".to_vec())],
    )
    .unwrap();
    assert!(apply_with_failure(&scratch.path, &plan, Some(100)).is_err());
    assert!(recover(&scratch.path, plan.id()).is_err());
    apply(&scratch.path, &plan).unwrap();
    remove(&scratch.path, plan.id()).unwrap();

    let second = preview(&scratch.path, [FileInput::new("other", b"owned".to_vec())]).unwrap();
    assert!(apply_with_failure(&scratch.path, &second, Some(101)).is_err());
    assert!(recover(&scratch.path, second.id()).is_err());
    assert!(remove(&scratch.path, second.id()).is_err());
    assert_eq!(fs::read(scratch.path.join("other")).unwrap(), b"owned");
    assert!(
        !scratch
            .path
            .join(".maestro-files")
            .join(ownership_name(second.id()))
            .exists()
    );
}

#[test]
fn a_stale_preview_never_overwrites_a_new_file() {
    let scratch = Scratch::new();
    let plan = preview(
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
    let plan = preview(
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
fn replay_preview_requires_complete_unchanged_ownership_and_applies_as_noop() {
    let scratch = Scratch::new();
    let inputs = [
        FileInput::new("one", b"one".to_vec()),
        FileInput::new("two", b"two".to_vec()),
    ];
    let first = preview(&scratch.path, inputs.clone()).unwrap();
    apply(&scratch.path, &first).unwrap();
    let replay = preview(&scratch.path, inputs.clone()).unwrap();
    assert!(replay.is_applied());
    apply(&scratch.path, &replay).unwrap();
    assert!(
        scratch
            .path
            .join(".maestro-files")
            .join(ownership_name(first.id()))
            .exists()
    );

    fs::remove_file(scratch.path.join("two")).unwrap();
    assert!(
        preview(&scratch.path, inputs.clone()).is_err(),
        "partial owned set accepted"
    );
    fs::write(scratch.path.join("two"), b"changed").unwrap();
    assert!(
        preview(&scratch.path, inputs).is_err(),
        "changed owned bytes accepted"
    );
}

#[test]
fn identical_unowned_files_and_different_plan_records_are_refused() {
    let scratch = Scratch::new();
    let input = FileInput::new("target", b"same".to_vec());
    let first = preview(&scratch.path, [input.clone()]).unwrap();
    fs::write(scratch.path.join("target"), b"same").unwrap();
    assert!(preview(&scratch.path, [input.clone()]).is_err());
    fs::remove_file(scratch.path.join("target")).unwrap();
    apply(&scratch.path, &first).unwrap();
    let changed_plan = FileInput::new("target", b"different".to_vec());
    assert!(preview(&scratch.path, [changed_plan]).is_err());
}

#[test]
fn path_validation_rejects_aliases_on_case_insensitive_and_windows_filesystems() {
    let scratch = Scratch::new();
    for path in [
        "dir/name:stream",
        "dir/trailing.",
        "dir/trailing ",
        "CON",
        "PRN.txt",
        "AUX.ext.more",
        "NUL",
        "COM1.txt",
        "COM9",
        "LPT1.txt",
        "LPT9",
        ".MAESTRO-FILES/state",
    ] {
        assert!(
            preview(&scratch.path, [FileInput::new(path, b"no".to_vec())]).is_err(),
            "accepted ambiguous path {path:?}"
        );
    }
    assert!(
        preview(
            &scratch.path,
            [
                FileInput::new("README.md", b"one".to_vec()),
                FileInput::new("readme.md", b"two".to_vec()),
            ]
        )
        .is_err()
    );
}

#[test]
fn traversal_and_reserved_paths_are_rejected_before_a_file_is_created() {
    let scratch = Scratch::new();
    for path in ["../outside", "a/../outside", ".maestro-files/journal"] {
        assert!(preview(&scratch.path, [FileInput::new(path, b"no".to_vec())]).is_err());
    }
    assert!(
        preview(
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

#[test]
fn replay_journal_recovers_after_owned_files_are_removed() {
    let scratch = Scratch::new();
    let inputs = [FileInput::new("target", b"planned".to_vec())];
    let first = preview(&scratch.path, inputs.clone()).unwrap();
    apply(&scratch.path, &first).unwrap();
    let replay = preview(&scratch.path, inputs).unwrap();
    assert!(replay.is_applied());
    remove(&scratch.path, replay.id()).unwrap();
    assert!(apply_with_failure(&scratch.path, &replay, Some(1)).is_err());
    recover(&scratch.path, replay.id()).unwrap();
    assert_eq!(fs::read(scratch.path.join("target")).unwrap(), b"planned");
}
