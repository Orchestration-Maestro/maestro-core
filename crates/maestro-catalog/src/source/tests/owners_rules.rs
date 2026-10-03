//! Source-rule admission and central standard exceptions bind real base reviewers.

use super::{
    area_packages::package_source,
    owners::{BASE, HEAD, REPOSITORY, fixture, snapshot, user},
    registry::glossary,
    support::MemoryTree,
};
use crate::{
    limits::Limits,
    source::{
        Known, Layout, Registry, builtin, frozen_rows,
        owners::{ApprovalFile, OwnerApproval, OwnerSnapshot, check_owners},
    },
};
use maestro_kernel::artifact::Digest;

/// Register a synthetic knowledge source with all four inert rule-family files.
fn source_snapshot(revision: &str, tree: &MemoryTree) -> OwnerSnapshot {
    let registry = source_registry();
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    OwnerSnapshot::check(
        revision,
        tree,
        &registry,
        &Limits::PRODUCTION,
        Known {
            rows: &rows,
            settings: &settings,
            today: 0,
        },
    )
    .unwrap()
}

#[test]
fn source_rule_owners_and_delegated_maintainers_bind_all_digests() {
    let tree = source_tree();
    let base = source_snapshot(BASE, &tree);
    let head = source_snapshot(HEAD, &tree);
    let (_, _, mut evidence) = fixture();
    evidence.approvals[0].files = head
        .files()
        .iter()
        .map(|(path, digest)| ApprovalFile {
            path: path.clone(),
            digest: Some(digest.clone()),
        })
        .collect();
    for actor in ["owner", "reader", "@OWNER"] {
        evidence.approvals[0].actor = actor.into();
        assert_eq!(
            check_owners(&base, &head, &evidence, REPOSITORY),
            Ok(()),
            "{actor}"
        );
    }
    for path in head
        .files()
        .keys()
        .filter(|path| path.starts_with("knowledge/"))
    {
        let mut altered = evidence.clone();
        altered.approvals[0].files.retain(|file| &file.path != path);
        assert!(
            check_owners(&base, &head, &altered, REPOSITORY)
                .unwrap_err()
                .contains(path)
        );
        let mut changed = evidence.clone();
        changed.approvals[0]
            .files
            .iter_mut()
            .find(|file| &file.path == path)
            .unwrap()
            .digest = Some(Digest::of(b"altered"));
        assert!(check_owners(&base, &head, &changed, REPOSITORY).is_err());
    }
    for mode in ["undelegated", "stale", "fabricated"] {
        let mut altered = evidence.clone();
        match mode {
            "undelegated" => altered.approvals[0].actor = "new-owner".into(),
            "stale" => altered.approvals[0].head = BASE.into(),
            _ => altered.approvals.clear(),
        }
        assert!(
            check_owners(&base, &head, &altered, REPOSITORY).is_err(),
            "{mode}"
        );
    }
}

#[test]
fn central_exception_requires_standard_and_root_reviews() {
    for path in [
        "exceptions/temporary.toml",
        "standards/security/exceptions/temporary.toml",
    ] {
        let (head_tree, base_tree) = exception_tree(path);
        let base = snapshot(BASE, &base_tree);
        let head = snapshot(HEAD, &head_tree);
        let (_, _, mut evidence) = fixture();
        evidence
            .principals
            .extend([user("root-owner"), user("standard-owner")]);
        evidence.approvals = ["root-owner", "standard-owner"]
            .map(|actor| OwnerApproval {
                actor: actor.into(),
                head: HEAD.into(),
                approved: true,
                files: vec![ApprovalFile {
                    path: path.into(),
                    digest: head.files().get(path).cloned(),
                }],
            })
            .to_vec();
        assert_eq!(check_owners(&base, &head, &evidence, REPOSITORY), Ok(()));
        for missing in [0, 1] {
            let mut altered = evidence.clone();
            altered.approvals.remove(missing);
            assert!(
                check_owners(&base, &head, &altered, REPOSITORY).is_err(),
                "{path}: {missing}"
            );
        }
    }
}

/// Root and standard owners protect a synthetic exception with valid metadata.
fn exception_tree(path: &str) -> (MemoryTree, MemoryTree) {
    let root = package_source("package", "common").replace("@synthetic/knowledge", "root-owner");
    let standard = package_source("standard", "security")
        .replace("@synthetic/knowledge", "standard-owner")
        .replace("security-001", "SEC-001");
    let tree = MemoryTree::owned()
        .with("package.toml", &root)
        .with("standards/security/package.toml", &standard);
    let metadata = standard.split_once("[metadata]").unwrap().1;
    let exception = format!(
        "name = \"temporary\"\n\
         rule = \"SEC-001\"\n\
         scopes = [\"package:core\"]\n\
         rationale = \"Synthetic only\"\n\
         expiry = \"2026-10-02\"\n\
         evidence = \"standard-check:security/approval\"\n\
         [metadata]{metadata}"
    );
    let base = tree.clone().with(path, &exception);
    (base, tree)
}

#[test]
fn expired_base_exception_deletion_uses_base_owner_evidence() {
    let (base_tree, head_tree) = exception_tree("exceptions/temporary.toml");
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    let registry = builtin().unwrap();
    let known = Known {
        rows: &rows,
        settings: &settings,
        today: 20_728,
    };
    let head =
        OwnerSnapshot::check(HEAD, &head_tree, &registry, &Limits::PRODUCTION, known).unwrap();
    let base =
        OwnerSnapshot::check_base(BASE, &base_tree, &registry, &Limits::PRODUCTION, known).unwrap();
    let (_, _, mut evidence) = fixture();
    evidence
        .principals
        .extend([user("root-owner"), user("standard-owner")]);
    evidence.approvals = ["root-owner", "standard-owner"]
        .map(|actor| OwnerApproval {
            actor: actor.into(),
            head: HEAD.into(),
            approved: true,
            files: vec![ApprovalFile {
                path: "exceptions/temporary.toml".into(),
                digest: None,
            }],
        })
        .to_vec();
    assert_eq!(check_owners(&base, &head, &evidence, REPOSITORY), Ok(()));
}

#[test]
fn expired_exception_kept_in_head_refuses() {
    let (tree, _) = exception_tree("exceptions/temporary.toml");
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    let registry = builtin().unwrap();
    let known = Known {
        rows: &rows,
        settings: &settings,
        today: 20_728,
    };
    OwnerSnapshot::check_base(BASE, &tree, &registry, &Limits::PRODUCTION, known).unwrap();
    let refusal =
        OwnerSnapshot::check(HEAD, &tree, &registry, &Limits::PRODUCTION, known).unwrap_err();
    assert!(refusal.diagnostics.iter().any(|diagnostic| {
        diagnostic.path == "exceptions/temporary.toml"
            && diagnostic.key == "expiry"
            && diagnostic.message == "expired central exception"
    }));
}

#[test]
fn malformed_base_exception_date_refuses() {
    let (tree, _) = exception_tree("exceptions/temporary.toml");
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    for date in ["not-a-date", "2026-02-30"] {
        let tree = tree
            .clone()
            .edit("exceptions/temporary.toml", "2026-10-02", date);
        let refusal = OwnerSnapshot::check_base(
            BASE,
            &tree,
            &builtin().unwrap(),
            &Limits::PRODUCTION,
            Known {
                rows: &rows,
                settings: &settings,
                today: 20_728,
            },
        )
        .unwrap_err();
        assert!(
            refusal.diagnostics.iter().any(|diagnostic| {
                diagnostic.path == "exceptions/temporary.toml"
                    && diagnostic.key == "expiry"
                    && diagnostic.message == "must be a YYYY-MM-DD calendar date"
            }),
            "{date}"
        );
    }
}

/// The synthetic registered kind is data, not another URL-rule parser.
fn source_registry() -> Registry {
    let mut registry = builtin().unwrap();
    let mut descriptor = glossary();
    descriptor.kind = "knowledge-source".into();
    descriptor.directory = "knowledge/sources".into();
    descriptor.layout = Layout::Folder {
        file: "source.toml".into(),
        data: [
            "policy.json",
            "decisions.json",
            "promotions.json",
            "migration.json",
        ]
        .map(str::to_owned)
        .to_vec(),
    };
    registry.register(descriptor).unwrap();
    registry
}

/// Exact synthetic rule inventory shared by ownership acceptance neighbours.
fn source_tree() -> MemoryTree {
    let root = package_source("package", "common")
        .replace("description =", "maintainers = [\"reader\"]\ndescription =");
    let metadata = root.split_once("[metadata]").unwrap().1;
    let mut tree = MemoryTree::owned().with("package.toml", &root).with(
        "knowledge/sources/site/source.toml",
        &format!("term = \"synthetic\"\n[metadata]{metadata}"),
    );
    for name in ["policy", "decisions", "promotions", "migration"] {
        tree = tree.with(&format!("knowledge/sources/site/{name}.json"), "{}");
    }
    tree
}

#[test]
fn owner_only_source_admission_and_fabricated_metadata_refuse() {
    let (_, _, mut evidence) = fixture();
    let mut owner_only = source_tree().edit("package.toml", "maintainers = [\"reader\"]\n", "");
    let owner_base = source_snapshot(BASE, &owner_only);
    let owner_head = source_snapshot(HEAD, &owner_only);
    evidence.approvals[0].actor = "owner".into();
    evidence.approvals[0].files = owner_head
        .files()
        .iter()
        .map(|(path, digest)| ApprovalFile {
            path: path.clone(),
            digest: Some(digest.clone()),
        })
        .collect();
    assert_eq!(
        check_owners(&owner_base, &owner_head, &evidence, REPOSITORY),
        Ok(())
    );
    owner_only = owner_only.edit(
        "knowledge/sources/site/source.toml",
        "term =",
        "reviewed_admission = true\nterm =",
    );
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    assert!(
        OwnerSnapshot::check(
            HEAD,
            &owner_only,
            &source_registry(),
            &Limits::PRODUCTION,
            Known {
                rows: &rows,
                settings: &settings,
                today: 0
            }
        )
        .is_err()
    );
}
