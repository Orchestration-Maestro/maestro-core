//! Strict evidence shapes, immutable snapshots and change-ownership boundaries.

use super::{
    area_packages::package_source,
    owners::{BASE, HEAD, REPOSITORY, fixture, snapshot, user},
    support::MemoryTree,
};
use crate::{
    limits::Limits,
    source::{
        Entry, Known, SourceTree, builtin, frozen_rows,
        owners::{ApprovalFile, OwnerEvidence, OwnerSnapshot, check_owners},
    },
};
use serde_json::json;
use std::{cell::Cell, collections::BTreeSet, io};

#[test]
fn owner_evidence_records_are_strict_objects() {
    let (_, _, evidence) = fixture();
    let value = serde_json::to_value(&evidence).unwrap();
    assert!(OwnerEvidence::parse(&serde_json::to_vec(&value).unwrap()).is_ok());
    for location in ["root", "principal", "review", "file"] {
        for shape in ["array", "unknown"] {
            let mut value = value.clone();
            let target = match location {
                "root" => &mut value,
                "principal" => &mut value["principals"][0],
                "review" => &mut value["approvals"][0],
                _ => &mut value["approvals"][0]["files"][0],
            };
            if shape == "array" {
                let keys: &[&str] = match location {
                    "root" => &[
                        "schema",
                        "repository",
                        "base",
                        "head",
                        "principals",
                        "approvals",
                    ],
                    "principal" => &["principal", "exists", "repository_access", "members"],
                    "review" => &["actor", "head", "approved", "files"],
                    _ => &["path", "digest"],
                };
                *target = keys
                    .iter()
                    .map(|key| target[*key].clone())
                    .collect::<Vec<_>>()
                    .into();
            } else {
                target["extra"] = json!(true);
            }
            assert!(
                OwnerEvidence::parse(&serde_json::to_vec(&value).unwrap()).is_err(),
                "{location} {shape}"
            );
        }
    }
    let bytes = serde_json::to_string(&value).unwrap();
    for replacement in [
        ("\"schema\":", "\"schema\":\"duplicate\",\"schema\":"),
        ("\"exists\":", "\"exists\":true,\"exists\":"),
        ("\"actor\":", "\"actor\":\"duplicate\",\"actor\":"),
        ("\"digest\":", "\"digest\":null,\"digest\":"),
    ] {
        assert!(
            OwnerEvidence::parse(bytes.replace(replacement.0, replacement.1).as_bytes()).is_err()
        );
    }
    for bad in [json!(false), json!(42), json!("nan"), json!("A".repeat(64))] {
        let mut value = value.clone();
        value["approvals"][0]["files"][0]["digest"] = bad;
        assert!(OwnerEvidence::parse(&serde_json::to_vec(&value).unwrap()).is_err());
    }
}

#[test]
fn malformed_identity_member_actor_and_path_refuse() {
    let (base, _, evidence) = fixture();
    let head = snapshot(HEAD, &MemoryTree::owned());
    for mode in [
        "principal",
        "member",
        "bad-member",
        "duplicate-member",
        "user-members",
        "actor",
        "team-actor",
        "review-head",
        "path",
        "empty-path",
    ] {
        let mut evidence = evidence.clone();
        match mode {
            "principal" => evidence.principals.push(user("bad name")),
            "member" => {
                evidence.principals[0].members = vec!["@other/team".into()];
                let mut team = evidence.principals[0].clone();
                team.principal = "@other/team".into();
                team.members = vec!["owner".into()];
                evidence.principals.push(team);
            }
            "bad-member" => evidence.principals[0].members = vec!["bad name".into()],
            "duplicate-member" => evidence.principals[0].members.push("@OWNER".into()),
            "user-members" => evidence.principals[1].members.push("reader".into()),
            "actor" => evidence.approvals[0].actor = "bad name".into(),
            "team-actor" => evidence.approvals[0].actor = "@synthetic/knowledge".into(),
            "review-head" => evidence.approvals[0].head = "main".into(),
            "path" => evidence.approvals[0].files[0].path = "../outside".into(),
            "empty-path" => evidence.approvals[0].files[0].path.clear(),
            _ => evidence.approvals[0].actor = "new-owner".into(),
        }
        assert!(
            check_owners(&base, &head, &evidence, REPOSITORY).is_err(),
            "{mode}"
        );
    }
    let (_, head, _) = fixture();
    let mut evidence = evidence;
    evidence.principals[0].members = vec!["reader".into()];
    assert!(check_owners(&base, &head, &evidence, REPOSITORY).is_err());
}

#[test]
fn new_area_and_deleted_descriptor_use_existing_base_owners() {
    let base_tree = MemoryTree::owned().edit(
        "package.toml",
        "description =",
        "maintainers = [\"reader\"]\ndescription =",
    );
    let base = snapshot(BASE, &base_tree);
    let (_, _, mut evidence) = fixture();
    let area = package_source("package", "new-area").replace("@synthetic/knowledge", "new-owner");
    let added = base_tree
        .clone()
        .with("capabilities/practice/new-area/package.toml", &area);
    let head = snapshot(HEAD, &added);
    let path = "capabilities/practice/new-area/package.toml";
    evidence.approvals[0].files = vec![ApprovalFile {
        path: path.into(),
        digest: head.files().get(path).cloned(),
    }];
    for actor in ["new-owner", "reader"] {
        evidence.approvals[0].actor = actor.into();
        assert!(
            check_owners(&base, &head, &evidence, REPOSITORY).is_err(),
            "{actor}"
        );
    }
    evidence.approvals[0].actor = "owner".into();
    assert_eq!(check_owners(&base, &head, &evidence, REPOSITORY), Ok(()));
    let head = snapshot(HEAD, &base_tree.without("core/package.toml"));
    evidence.approvals[0].files = vec![ApprovalFile {
        path: "core/package.toml".into(),
        digest: None,
    }];
    assert_eq!(check_owners(&base, &head, &evidence, REPOSITORY), Ok(()));
    evidence.approvals[0].files[0].digest = base.files().get("core/package.toml").cloned();
    assert!(check_owners(&base, &head, &evidence, REPOSITORY).is_err());
}

#[test]
fn ordinary_content_uses_base_not_newly_delegated_maintainers() {
    let tree = MemoryTree::valid();
    let base = snapshot(BASE, &tree);
    let head = snapshot(
        HEAD,
        &tree
            .clone()
            .edit(
                "core/package.toml",
                "description =",
                "maintainers = [\"reader\"]\ndescription =",
            )
            .edit(
                "core/agents/valid.agent.md",
                "## Boundaries",
                "Changed synthetic content.\n\n## Boundaries",
            ),
    );
    let (_, _, mut evidence) = fixture();
    evidence.approvals[0].files = head
        .files()
        .iter()
        .map(|(path, digest)| ApprovalFile {
            path: path.clone(),
            digest: Some(digest.clone()),
        })
        .collect();
    let mut delegated = evidence.approvals[0].clone();
    delegated.actor = "reader".into();
    delegated
        .files
        .retain(|file| file.path != "core/package.toml");
    evidence.approvals[0]
        .files
        .retain(|file| file.path == "core/package.toml");
    evidence.approvals.push(delegated);
    assert!(check_owners(&base, &head, &evidence, REPOSITORY).is_err());
    let base_tree = tree.edit(
        "core/package.toml",
        "description =",
        "maintainers = [\"reader\"]\ndescription =",
    );
    let base = snapshot(BASE, &base_tree);
    evidence.approvals[0].files.clear();
    assert_eq!(check_owners(&base, &head, &evidence, REPOSITORY), Ok(()));
}

/// Refuse a second source read so decoded ownership and digests cannot race.
struct OnceTree {
    /// Catalog bytes checked on first read.
    tree: MemoryTree,
    /// Reads of the root descriptor.
    reads: Cell<usize>,
}
impl SourceTree for OnceTree {
    fn list(&self, directory: &str) -> io::Result<Vec<Entry>> {
        self.tree.list(directory)
    }
    fn read(&self, path: &str, max_bytes: u64) -> io::Result<Vec<u8>> {
        if path == "package.toml" {
            self.reads.set(self.reads.get() + 1);
            assert_eq!(
                self.reads.get(),
                1,
                "owner snapshot reopened validated bytes"
            );
        }
        self.tree.read(path, max_bytes)
    }
}

#[test]
fn immutable_snapshot_reads_once_and_refuses_mutable_revision() {
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    let known = Known {
        rows: &rows,
        settings: &settings,
        today: 0,
    };
    let tree = OnceTree {
        tree: MemoryTree::owned(),
        reads: Cell::new(0),
    };
    let checked =
        OwnerSnapshot::check(BASE, &tree, &builtin().unwrap(), &Limits::PRODUCTION, known).unwrap();
    assert_eq!(tree.reads.get(), 1);
    assert_eq!(checked.files().len(), 2);
    for revision in [
        "main".into(),
        "A".repeat(40),
        "g".repeat(40),
        "1".repeat(39),
        "1".repeat(41),
        String::new(),
    ] {
        assert!(
            OwnerSnapshot::check(
                &revision,
                &MemoryTree::owned(),
                &builtin().unwrap(),
                &Limits::PRODUCTION,
                known
            )
            .is_err()
        );
    }
    assert!(
        OwnerSnapshot::check(
            &"a".repeat(64),
            &MemoryTree::owned(),
            &builtin().unwrap(),
            &Limits::PRODUCTION,
            known
        )
        .is_ok()
    );
}

#[test]
fn missing_root_or_empty_repository_cannot_authorize_admission() {
    let (base, head, mut evidence) = fixture();
    let empty_base = snapshot(BASE, &MemoryTree::owned().without("package.toml"));
    let empty_head = snapshot(HEAD, &MemoryTree::owned().without("package.toml"));
    assert!(check_owners(&empty_base, &head, &evidence, REPOSITORY).is_err());
    let mut deleting = evidence.clone();
    deleting.approvals[0].files = vec![ApprovalFile {
        path: "package.toml".into(),
        digest: None,
    }];
    assert!(check_owners(&base, &empty_head, &deleting, REPOSITORY).is_err());
    assert!(check_owners(&empty_base, &empty_head, &evidence, REPOSITORY).is_err());
    evidence.repository.clear();
    assert!(check_owners(&base, &head, &evidence, "").is_err());
}

#[test]
fn root_governance_cannot_be_reviewed_by_content_delegate() {
    let tree = MemoryTree::valid().edit(
        "package.toml",
        "description =",
        "maintainers = [\"reader\"]\ndescription =",
    );
    let base = snapshot(BASE, &tree);
    let head = snapshot(
        HEAD,
        &tree.edit(
            "presets/knowledge-client.toml",
            "[metadata]",
            "[metadata]\nversion = \"1.2.3\"",
        ),
    );
    let (_, _, mut evidence) = fixture();
    evidence.approvals[0].files = vec![ApprovalFile {
        path: "presets/knowledge-client.toml".into(),
        digest: head.files().get("presets/knowledge-client.toml").cloned(),
    }];
    evidence.approvals[0].actor = "reader".into();
    assert!(check_owners(&base, &head, &evidence, REPOSITORY).is_err());
    evidence.approvals[0].actor = "owner".into();
    assert_eq!(check_owners(&base, &head, &evidence, REPOSITORY), Ok(()));
}

#[test]
fn direct_user_handles_and_stale_review_neighbours_accept() {
    let tree = MemoryTree::owned().edit("core/package.toml", "@synthetic/knowledge", "@OWNER");
    let base = snapshot(BASE, &tree);
    let head = snapshot(
        HEAD,
        &tree.edit(
            "core/package.toml",
            "Synthetic area",
            "Changed synthetic area",
        ),
    );
    let (_, _, mut evidence) = fixture();
    evidence.approvals[0].files = head
        .files()
        .iter()
        .map(|(path, digest)| ApprovalFile {
            path: path.clone(),
            digest: Some(digest.clone()),
        })
        .collect();
    let mut stale = evidence.approvals[0].clone();
    stale.head = BASE.into();
    evidence.approvals.insert(0, stale);
    assert_eq!(check_owners(&base, &head, &evidence, REPOSITORY), Ok(()));
    evidence
        .approvals
        .iter_mut()
        .for_each(|review| review.actor = "reader".into());
    assert!(check_owners(&base, &head, &evidence, REPOSITORY).is_err());
}

#[test]
fn unused_team_delegation_still_requires_readiness_and_member_access() {
    let tree = MemoryTree::owned().edit(
        "core/package.toml",
        "description =",
        "maintainers = [\"@synthetic/delegates\"]\ndescription =",
    );
    let base = snapshot(BASE, &tree);
    let head = snapshot(HEAD, &tree);
    let (_, _, mut evidence) = fixture();
    let mut team = evidence.principals[0].clone();
    team.principal = "@synthetic/delegates".into();
    team.members.clear();
    evidence.principals.push(team);
    evidence.approvals.clear();
    assert!(check_owners(&base, &head, &evidence, REPOSITORY).is_err());
    evidence
        .principals
        .last_mut()
        .unwrap()
        .members
        .push("new-owner".into());
    evidence
        .principals
        .iter_mut()
        .find(|record| record.principal == "new-owner")
        .unwrap()
        .repository_access = false;
    assert!(check_owners(&base, &head, &evidence, REPOSITORY).is_err());
    evidence
        .principals
        .iter_mut()
        .find(|record| record.principal == "new-owner")
        .unwrap()
        .repository_access = true;
    assert_eq!(check_owners(&base, &head, &evidence, REPOSITORY), Ok(()));
}

#[test]
fn new_area_content_needs_central_owner_not_root_delegate() {
    let tree = MemoryTree::owned().edit(
        "package.toml",
        "description =",
        "maintainers = [\"reader\"]\ndescription =",
    );
    let base = snapshot(BASE, &tree);
    let descriptor = "capabilities/practice/new-area/package.toml";
    let content = "capabilities/practice/new-area/skills/valid-skill/SKILL.md";
    let area = package_source("package", "new-area").replace("@synthetic/knowledge", "new-owner");
    let head_tree = tree.with(descriptor, &area).with(
        content,
        &MemoryTree::valid().text("skills/valid-skill/SKILL.md"),
    );
    let head = snapshot(HEAD, &head_tree);
    let (_, _, mut evidence) = fixture();
    evidence.approvals[0].files = vec![ApprovalFile {
        path: descriptor.into(),
        digest: head.files().get(descriptor).cloned(),
    }];
    let mut delegate = evidence.approvals[0].clone();
    delegate.actor = "reader".into();
    delegate.files = vec![ApprovalFile {
        path: content.into(),
        digest: head.files().get(content).cloned(),
    }];
    evidence.approvals.push(delegate);
    assert!(check_owners(&base, &head, &evidence, REPOSITORY).is_err());
    evidence.approvals[1].actor = "owner".into();
    assert_eq!(check_owners(&base, &head, &evidence, REPOSITORY), Ok(()));
}

#[test]
fn moved_area_preserves_base_identity_and_protects_new_path() {
    let old = "capabilities/practice/review";
    let new = "capabilities/workflow/review";
    let area = package_source("package", "review")
        .replace("@synthetic/knowledge", "new-owner")
        .replace("description =", "maintainers = [\"reader\"]\ndescription =");
    let skill = MemoryTree::valid().text("skills/valid-skill/SKILL.md");
    let tree = MemoryTree::owned()
        .with(&format!("{old}/package.toml"), &area)
        .with(&format!("{old}/skills/valid-skill/SKILL.md"), &skill);
    let base = snapshot(BASE, &tree);
    let head_tree = tree
        .without(&format!("{old}/package.toml"))
        .without(&format!("{old}/skills/valid-skill/SKILL.md"))
        .with(&format!("{new}/package.toml"), &area)
        .with(&format!("{new}/skills/valid-skill/SKILL.md"), &skill);
    let head = snapshot(HEAD, &head_tree);
    let (_, _, mut evidence) = fixture();
    evidence.approvals[0].actor = "new-owner".into();
    evidence.approvals[0].files = base
        .files()
        .keys()
        .chain(head.files().keys())
        .filter(|path| path.ends_with("package.toml"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|path| ApprovalFile {
            path: path.clone(),
            digest: head.files().get(path).cloned(),
        })
        .collect();
    let mut delegate = evidence.approvals[0].clone();
    delegate.actor = "reader".into();
    delegate.files = [
        format!("{old}/skills/valid-skill/SKILL.md"),
        format!("{new}/skills/valid-skill/SKILL.md"),
    ]
    .map(|path| ApprovalFile {
        digest: head.files().get(&path).cloned(),
        path,
    })
    .to_vec();
    evidence.approvals.push(delegate);
    assert!(check_owners(&base, &head, &evidence, REPOSITORY).is_err());
    evidence.approvals[1].actor = "new-owner".into();
    assert_eq!(check_owners(&base, &head, &evidence, REPOSITORY), Ok(()));
}
