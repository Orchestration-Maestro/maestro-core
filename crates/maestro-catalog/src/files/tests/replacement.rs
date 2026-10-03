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

#[test]
fn files_replacement_journal_guards() {
    use crate::files::{digest, transition::ReplacementPlan};
    let scratch = Scratch::new();
    let root = &scratch.path;
    with_trust(root, |trust| {
        let plan =
            FilePlan::preview_replacement(root, "shared.json", b"new".to_vec(), trust).unwrap();
        assert!(plan.changed());
        assert!(files::apply_with_failure(root, &plan, Some(0), trust).is_err());
        let journal = root.join(format!(
            ".maestro-files/replace-{}.toml",
            plan.id().strip_prefix("sha256:").unwrap()
        ));
        let original = fs::read_to_string(&journal).unwrap();
        assert!(
            toml::from_str::<ReplacementPlan>(&format!("{original}\nunknown = true\n")).is_err()
        );
        let other_id = digest(b"other");
        let other = root.join(format!(
            ".maestro-files/replace-{}.toml",
            other_id.strip_prefix("sha256:").unwrap()
        ));
        fs::write(&other, &original).unwrap();
        assert!(FilePlan::recover_replacement(root, &other_id, trust).is_err());
        assert!(!root.join("shared.json").exists());
        fs::remove_file(other).unwrap();
        fs::write(&journal, format!("{original}\n")).unwrap();
        assert!(files::apply(root, &plan, trust).is_err());
        assert!(!root.join("shared.json").exists());
        let record: ReplacementPlan = toml::from_str(&original).unwrap();
        for schema_only in [true, false] {
            let mut changed = record.clone();
            if schema_only {
                changed.schema = "maestro-replacement/2".to_owned();
                changed.id.clear();
                changed.id = digest(toml::to_string(&changed).unwrap().as_bytes());
            } else {
                changed.new = b"altered".to_vec();
            }
            fs::write(&journal, toml::to_string(&changed).unwrap()).unwrap();
            assert!(FilePlan::recover_replacement(root, plan.id(), trust).is_err());
            assert!(!root.join("shared.json").exists());
        }
        fs::write(&journal, &original).unwrap();
        FilePlan::recover_replacement(root, plan.id(), trust).unwrap();
        assert_eq!(fs::read(root.join("shared.json")).unwrap(), b"new");
        assert!(!journal.exists());
    });
}

#[test]
fn files_replacement_boundaries_and_noop_effects() {
    use crate::limits::Limits;
    let scratch = Scratch::new();
    let root = &scratch.path;
    let limit = usize::try_from(Limits::PRODUCTION.source_file_bytes).unwrap();
    with_trust(root, |trust| {
        for length in [limit, limit + 1] {
            let bytes = vec![b'x'; length];
            fs::write(root.join("shared.json"), &bytes).unwrap();
            let result = FilePlan::preview_replacement(root, "shared.json", b"new".to_vec(), trust);
            assert_eq!(result.is_ok(), length == limit);
            fs::remove_file(root.join("shared.json")).unwrap();
            let result = FilePlan::preview_replacement(root, "shared.json", bytes, trust);
            assert_eq!(result.is_ok(), length == limit);
        }
        assert!(!root.join(".maestro-files").exists());
        fs::write(root.join("shared.json"), b"unchanged").unwrap();
        let noop = FilePlan::preview_replacement(root, "shared.json", b"unchanged".to_vec(), trust)
            .unwrap();
        assert!(!noop.changed());
        files::apply(root, &noop, trust).unwrap();
        assert!(!root.join(".maestro-files").exists());
        assert_eq!(fs::read(root.join("shared.json")).unwrap(), b"unchanged");
    });
}

#[test]
fn workspace_replacement_effect_fences() {
    use crate::policy::workspace::{Access, CheckedTrust, TrustBoundaries, WorkspaceTrust};
    use std::path::PathBuf;
    struct Authority {
        root: PathBuf,
        after_publication: bool,
    }
    impl WorkspaceTrust for Authority {
        fn containing_root(&self, path: &Path) -> Option<PathBuf> {
            let revoke = if self.after_publication {
                fs::read(self.root.join("shared.json")).unwrap() == b"new"
            } else {
                fs::read_dir(&self.root)
                    .unwrap()
                    .map(Result::unwrap)
                    .map(|entry| entry.file_name())
                    .any(|name| name.to_string_lossy().starts_with(".maestro-replace-"))
            };
            (!revoke && path.starts_with(&self.root)).then(|| self.root.clone())
        }
    }
    for after_publication in [false, true] {
        let scratch = Scratch::new();
        let root = scratch.path.canonicalize().unwrap();
        fs::write(root.join("shared.json"), b"old").unwrap();
        let authority = Authority {
            root: root.clone(),
            after_publication,
        };
        let boundaries = TrustBoundaries::new(root.parent().unwrap(), &[]).unwrap();
        let trust = CheckedTrust::new(&authority, &boundaries);
        let grant = trust
            .authorize(&root, Path::new("shared.json"), Access::Write)
            .unwrap();
        assert!(grant.replace_verified(b"old", b"new").is_err());
        assert_eq!(
            fs::read(root.join("shared.json")).unwrap(),
            if after_publication { b"new" } else { b"old" }
        );
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    }
}

#[test]
fn workspace_replacement_revocation_precedes_staged_contents() {
    use crate::policy::workspace::{Access, CheckedTrust, TrustBoundaries, WorkspaceTrust};
    use std::{
        cell::{Cell, RefCell},
        path::PathBuf,
    };
    struct Authority {
        root: PathBuf,
        approved: Cell<bool>,
        staged_bytes: RefCell<Vec<Vec<u8>>>,
    }
    impl WorkspaceTrust for Authority {
        fn containing_root(&self, path: &Path) -> Option<PathBuf> {
            for entry in fs::read_dir(&self.root)
                .unwrap()
                .map(Result::unwrap)
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_string_lossy()
                        .starts_with(".maestro-replace-")
                })
            {
                self.staged_bytes
                    .borrow_mut()
                    .push(fs::read(entry.path()).unwrap());
            }
            (self.approved.get() && path.starts_with(&self.root)).then(|| self.root.clone())
        }
    }
    let scratch = Scratch::new();
    let root = scratch.path.canonicalize().unwrap();
    fs::write(root.join("shared.json"), b"old").unwrap();
    let authority = Authority {
        root: root.clone(),
        approved: Cell::new(true),
        staged_bytes: RefCell::new(Vec::new()),
    };
    let boundaries = TrustBoundaries::new(root.parent().unwrap(), &[]).unwrap();
    let trust = CheckedTrust::new(&authority, &boundaries);
    let grant = trust
        .authorize(&root, Path::new("shared.json"), Access::Write)
        .unwrap();
    authority.approved.set(false);
    assert!(grant.replace_verified(b"old", b"new").is_err());
    assert!(
        authority.staged_bytes.borrow().is_empty(),
        "revoked authority must precede even transient staging writes"
    );
    assert_eq!(fs::read(root.join("shared.json")).unwrap(), b"old");
    assert_eq!(fs::read_dir(root).unwrap().count(), 1);
}
