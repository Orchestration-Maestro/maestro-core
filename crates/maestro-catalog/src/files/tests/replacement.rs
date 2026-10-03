//! C04 replacement plans capture old bytes and recover only their journaled transition.
use super::support::{Scratch, with_trust};
use crate::files::{self, FilePlan};
use std::{fs, path::Path};

#[test]
fn files_replacement_preview_drift_and_recovery() {
    let scratch = Scratch::new();
    let root = &scratch.path;
    fs::write(root.join("shared.json"), b"old").unwrap();
    fs::write(root.join("neighbour"), b"user").unwrap();
    with_trust(root, |trust| {
        let plan =
            FilePlan::preview_replacement(root, "shared.json", b"new".to_vec(), trust).unwrap();
        assert!(!root.join(".maestro-files").exists());
        fs::write(root.join("shared.json"), b"edited").unwrap();
        assert!(files::apply(root, &plan, trust).is_err());
        assert_eq!(fs::read(root.join("shared.json")).unwrap(), b"edited");
        assert!(!root.join(".maestro-files").exists());
        fs::write(root.join("shared.json"), b"old").unwrap();
        assert!(files::apply_with_failure(root, &plan, Some(0), trust).is_err());
        assert_eq!(fs::read(root.join("shared.json")).unwrap(), b"old");
        FilePlan::recover_replacement(root, plan.id(), trust).unwrap();
        assert_eq!(fs::read(root.join("shared.json")).unwrap(), b"new");
        let plan =
            FilePlan::preview_replacement(root, "shared.json", b"last".to_vec(), trust).unwrap();
        assert!(files::apply_with_failure(root, &plan, Some(1), trust).is_err());
        assert_eq!(fs::read(root.join("shared.json")).unwrap(), b"last");
        FilePlan::recover_replacement(root, plan.id(), trust).unwrap();
        assert_eq!(fs::read(root.join("neighbour")).unwrap(), b"user");
    });
}

#[test]
fn files_replacement_create_noop_and_invalid_recovery() {
    let scratch = Scratch::new();
    with_trust(&scratch.path, |trust| {
        let plan =
            FilePlan::preview_replacement(&scratch.path, "shared.json", b"new".to_vec(), trust)
                .unwrap();
        files::apply(&scratch.path, &plan, trust).unwrap();
        let noop =
            FilePlan::preview_replacement(&scratch.path, "shared.json", b"new".to_vec(), trust)
                .unwrap();
        assert!(!noop.changed());
        files::apply(&scratch.path, &noop, trust).unwrap();
        assert!(FilePlan::recover_replacement(&scratch.path, "../bad", trust).is_err());
        assert!(FilePlan::preview_replacement(&scratch.path, "../bad", Vec::new(), trust).is_err());
        assert!(
            FilePlan::preview_replacement(&scratch.path, ".maestro-files/bad", Vec::new(), trust)
                .is_err()
        );
        assert!(FilePlan::recover_replacement(Path::new("missing"), plan.id(), trust).is_err());
    });
}
