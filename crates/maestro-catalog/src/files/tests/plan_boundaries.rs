//! Hostile persisted records and exact ownership comparisons.
use super::support::{Scratch, apply, apply_with_failure, preview, with_trust};
use crate::{
    files::{
        FileInput, FilePlan, effects,
        names::{journal_name, ownership_name},
        plan::{digest, ownership_matches, validate_id, validate_plan, validate_relative_path},
        recovery::read_optional,
    },
    limits::Limits,
};
use std::{fs, io, str};
use toml::map::Map;

#[test]
fn ownership_requires_identity_count_path_and_digest() {
    let scratch = Scratch::new();
    let plan = preview(
        &scratch.path,
        [
            FileInput::new("one", b"one".to_vec()),
            FileInput::new("two", b"two".to_vec()),
        ],
    )
    .unwrap();
    apply(&scratch.path, &plan).unwrap();
    let path = scratch
        .path
        .join(".maestro-files")
        .join(ownership_name(plan.id()));
    let original: toml::Value = toml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    for field in ["id", "count", "path", "digest"] {
        let mut record = original.clone();
        match field {
            "id" => record["id"] = toml::Value::String(digest(b"another plan")),
            "count" => {
                record["files"].as_array_mut().unwrap().pop();
            }
            "path" => record["files"][0]["path"] = toml::Value::String("different".into()),
            _ => record["files"][0]["digest"] = toml::Value::String(digest(b"different")),
        }
        let bytes = toml::to_string(&record).unwrap().into_bytes();
        assert!(!ownership_matches(&bytes, &plan).unwrap(), "{field}");
        fs::write(&path, bytes).unwrap();
        assert!(
            apply(&scratch.path, &plan)
                .unwrap_err()
                .to_string()
                .contains("conflicting ownership"),
            "{field}"
        );
    }
}

#[test]
fn apply_rechecks_owned_bytes_and_refuses_conflicting_journal() {
    let scratch = Scratch::new();
    let plan = preview(&scratch.path, [FileInput::new("target", b"owned".to_vec())]).unwrap();
    assert!(!plan.is_applied());
    apply_with_failure(&scratch.path, &plan, Some(0)).unwrap_err();
    let journal = scratch
        .path
        .join(".maestro-files")
        .join(journal_name(plan.id()));
    fs::write(&journal, b"other journal").unwrap();
    assert!(
        apply(&scratch.path, &plan)
            .unwrap_err()
            .to_string()
            .contains("conflicting file journal")
    );
    fs::remove_file(journal).unwrap();
    apply(&scratch.path, &plan).unwrap();
    fs::write(scratch.path.join("target"), b"changed").unwrap();
    assert!(
        apply(&scratch.path, &plan)
            .unwrap_err()
            .to_string()
            .contains("owned file changed")
    );
}

#[test]
fn crash_points_after_three_files_distinguish_commit_from_cleanup() {
    for (point, committed) in [(7, false), (8, true)] {
        let scratch = Scratch::new();
        let plan = preview(
            &scratch.path,
            [
                FileInput::new("one", b"1".to_vec()),
                FileInput::new("two", b"2".to_vec()),
                FileInput::new("three", b"3".to_vec()),
            ],
        )
        .unwrap();
        let error = apply_with_failure(&scratch.path, &plan, Some(point)).unwrap_err();
        assert!(error.to_string().contains("injected"));
        assert_eq!(
            scratch
                .path
                .join(".maestro-files")
                .join(ownership_name(plan.id()))
                .exists(),
            committed
        );
        assert!(
            scratch
                .path
                .join(".maestro-files")
                .join(journal_name(plan.id()))
                .exists()
        );
    }
}

#[test]
fn portable_path_validation_accepts_nondevices_and_refuses_each_alias() {
    for path in [
        "ordinary", "COM0", "COM10", "LPT0", "LPT10", "XOM1", "dir/file",
    ] {
        validate_relative_path(path).unwrap_or_else(|error| panic!("{path}: {error}"));
    }
    for path in [
        "",
        "a\\b",
        ".maestro-files/state",
        "a//b",
        "a/./b",
        ".",
        "..",
        "a/../b",
        "name:stream",
        "name.",
        "name ",
    ] {
        assert!(validate_relative_path(path).is_err(), "{path:?}");
    }
    for id in ["", "sha256:abc", "sha256:../../outside"] {
        assert!(validate_id(id).is_err(), "{id}");
    }
}

#[test]
fn journal_validation_checks_nonempty_identity_and_content() {
    let scratch = Scratch::new();
    let plan = preview(&scratch.path, [FileInput::new("target", b"owned".to_vec())]).unwrap();
    for change in ["empty", "identity", "content"] {
        let mut changed = plan.clone();
        match change {
            "empty" => changed.entries.clear(),
            "identity" => changed.id = digest(b"another identity"),
            _ => changed.entries[0].bytes = b"changed".to_vec(),
        }
        assert!(validate_plan(&changed).is_err(), "{change}");
    }
}

#[test]
fn optional_reads_and_replay_propagate_non_not_found_errors() {
    let scratch = Scratch::new();
    let inputs = [FileInput::new("target", b"owned".to_vec())];
    let plan = preview(&scratch.path, inputs.clone()).unwrap();
    fs::create_dir_all(
        scratch
            .path
            .join(".maestro-files")
            .join(ownership_name(plan.id())),
    )
    .unwrap();
    with_trust(&scratch.path, |trust| {
        assert!(
            read_optional(
                &scratch.path,
                &format!(".maestro-files/{}", ownership_name(plan.id())),
                trust
            )
            .is_err()
        );
    });
    assert!(preview(&scratch.path, inputs).is_err());
}

#[test]
fn apply_preserves_target_read_errors_before_crash_injection() {
    use crate::{
        files::apply_with_failure as apply_checked,
        policy::workspace::{CheckedTrust, TrustBoundaries, WorkspaceTrust},
    };
    use std::{
        cell::Cell,
        path::{Path, PathBuf},
    };

    // Introduce a directory only after preflight, when publication reaches its state parent.
    struct ChangedTarget {
        root: PathBuf,
        changed: Cell<bool>,
    }
    impl WorkspaceTrust for ChangedTarget {
        fn containing_root(&self, path: &Path) -> Option<PathBuf> {
            if path
                .file_name()
                .is_some_and(|name| name == ".maestro-files")
                && !self.changed.replace(true)
            {
                fs::create_dir(self.root.join("target")).unwrap();
            }
            path.starts_with(&self.root).then(|| self.root.clone())
        }
    }
    let scratch = Scratch::new();
    let root = scratch.path.canonicalize().unwrap();
    let plan = preview(&root, [FileInput::new("target", b"owned".to_vec())]).unwrap();
    let boundaries = TrustBoundaries::new(root.parent().unwrap(), &[]).unwrap();
    let adapter = ChangedTarget {
        root: root.clone(),
        changed: Cell::new(false),
    };
    let trust = CheckedTrust::new(&adapter, &boundaries);
    let error = apply_checked(&root, &plan, Some(1), &trust).unwrap_err();
    assert!(adapter.changed.get());
    assert!(!error.to_string().contains("injected"), "{error}");
}

#[test]
fn committed_file_bounds_count_name_and_aggregate_records() {
    let scratch = Scratch::new();
    let root = scratch.path.canonicalize().unwrap();
    let inputs = [
        FileInput::new("lock", b"lock".to_vec()),
        FileInput::new("other", b"other".to_vec()),
    ];
    let plan = preview(&root, inputs).unwrap();
    apply(&root, &plan).unwrap();
    let state = root.join(".maestro-files");
    let record_path = state.join(ownership_name(plan.id()));
    let original = fs::read(&record_path).unwrap();
    let mut record: toml::Value = toml::from_str(str::from_utf8(&original).unwrap()).unwrap();
    with_trust(&root, |trust| {
        let limits = Limits {
            archive_entries: 2,
            ..Limits::PRODUCTION
        };
        assert_eq!(
            FilePlan::committed_file(&root, "lock", b"lock", trust, &limits)
                .unwrap()
                .len(),
            2
        );
        record["files"]
            .as_array_mut()
            .unwrap()
            .push(toml::Value::Table(Map::from_iter([
                ("path".into(), toml::Value::String("third".into())),
                ("digest".into(), toml::Value::String(digest(b"3"))),
            ])));
        fs::write(&record_path, toml::to_string(&record).unwrap()).unwrap();
        assert!(
            FilePlan::committed_file(&root, "lock", b"lock", trust, &limits)
                .unwrap_err()
                .to_string()
                .contains("invalid committed")
        );
        fs::write(&record_path, &original).unwrap();
        let wrong_name = state.join(ownership_name(&digest(b"wrong")));
        fs::rename(&record_path, &wrong_name).unwrap();
        assert!(
            FilePlan::committed_file(&root, "lock", b"lock", trust, &Limits::PRODUCTION)
                .unwrap_err()
                .to_string()
                .contains("invalid committed")
        );
        fs::rename(&wrong_name, &record_path).unwrap();
        let second = preview(&root, [FileInput::new("separate", b"separate".to_vec())]).unwrap();
        apply(&root, &second).unwrap();
        let total: u64 = fs::read_dir(&state)
            .unwrap()
            .map(|entry| entry.unwrap().metadata().unwrap().len())
            .sum();
        let limits = Limits {
            archive_total_bytes: total - 1,
            ..Limits::PRODUCTION
        };
        let error = FilePlan::committed_file(&root, "lock", b"lock", trust, &limits).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Other);
        assert!(error.to_string().contains("bytes exceed limit"), "{error}");
    });
}

#[test]
fn effects_preflight_refuses_secret_outputs_before_any_write() {
    let scratch = Scratch::new();
    with_trust(&scratch.path, |trust| {
        assert!(effects::check(&scratch.path, ".env", trust).is_err());
    });
    assert_eq!(fs::read_dir(&scratch.path).unwrap().count(), 0);
}

#[test]
fn missing_committed_directory_has_the_specific_ownership_diagnostic() {
    let scratch = Scratch::new();
    let root = scratch.path.canonicalize().unwrap();
    with_trust(&root, |trust| {
        let error = FilePlan::committed_file(&root, "lock", b"lock", trust, &Limits::PRODUCTION)
            .unwrap_err();
        assert_eq!(error.to_string(), "no committed ownership record");
    });
}
