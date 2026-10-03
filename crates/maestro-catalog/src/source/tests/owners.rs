//! Trusted synthetic lookups and reviews: no API or credential in these checks.

use super::support::MemoryTree;
use crate::{
    limits::Limits,
    source::{
        Known, builtin, frozen_rows,
        owners::{
            ApprovalFile, OwnerApproval, OwnerEvidence, OwnerSnapshot, PrincipalEvidence,
            check_owners,
        },
    },
};

/// Full immutable commits, not branch names.
pub(super) const BASE: &str = "1111111111111111111111111111111111111111";
/// Proposed immutable commit.
pub(super) const HEAD: &str = "2222222222222222222222222222222222222222";
/// Repository selected outside catalog content.
pub(super) const REPOSITORY: &str = "synthetic/manifests";

/// Check injected bytes with the production source parser and bounds.
pub(super) fn snapshot(revision: &str, tree: &MemoryTree) -> OwnerSnapshot {
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().unwrap();
    OwnerSnapshot::check(
        revision,
        tree,
        &builtin().unwrap(),
        &Limits::PRODUCTION,
        Known {
            rows: &rows,
            settings: &settings,
            today: 0,
        },
    )
    .unwrap()
}

/// Existing team owner and delegated user; the descriptor changes its description.
pub(super) fn fixture() -> (OwnerSnapshot, OwnerSnapshot, OwnerEvidence) {
    let tree = MemoryTree::owned();
    let head_tree = tree.clone().edit(
        "core/package.toml",
        "description =",
        "maintainers = [\"reader\"]\ndescription =",
    );
    let base = snapshot(BASE, &tree);
    let head = snapshot(HEAD, &head_tree);
    let evidence = OwnerEvidence {
        schema: "maestro-owner-evidence/1".into(),
        repository: REPOSITORY.into(),
        base: BASE.into(),
        head: HEAD.into(),
        principals: vec![
            PrincipalEvidence {
                principal: "@synthetic/knowledge".into(),
                exists: true,
                repository_access: true,
                members: vec!["owner".into()],
            },
            user("owner"),
            user("reader"),
            user("new-owner"),
        ],
        approvals: vec![OwnerApproval {
            actor: "owner".into(),
            head: HEAD.into(),
            approved: true,
            files: head
                .files()
                .iter()
                .map(|(path, digest)| ApprovalFile {
                    path: path.clone(),
                    digest: Some(digest.clone()),
                })
                .collect(),
        }],
    };
    (base, head, evidence)
}

/// Successful user lookup; team membership is always separate.
pub(super) fn user(principal: &str) -> PrincipalEvidence {
    PrincipalEvidence {
        principal: principal.into(),
        exists: true,
        repository_access: true,
        members: vec![],
    }
}

#[test]
fn verified_existing_principals_accept() {
    let (base, head, evidence) = fixture();
    assert_eq!(check_owners(&base, &head, &evidence, REPOSITORY), Ok(()));
}

#[test]
fn unknown_team_or_missing_access_refuses() {
    let (base, head, evidence) = fixture();
    for index in [0, 1, 2] {
        for field in ["exists", "access", "missing"] {
            let mut evidence = evidence.clone();
            match field {
                "exists" => evidence.principals[index].exists = false,
                "access" => evidence.principals[index].repository_access = false,
                _ => {
                    evidence.principals.remove(index);
                }
            }
            assert!(
                check_owners(&base, &head, &evidence, REPOSITORY).is_err(),
                "{index} {field}"
            );
        }
    }
    let mut evidence = evidence;
    evidence.principals[0].members.clear();
    assert!(check_owners(&base, &head, &evidence, REPOSITORY).is_err());
}

#[test]
fn stale_head_approval_refuses() {
    let (base, head, mut evidence) = fixture();
    evidence.approvals[0].head = BASE.into();
    let result = check_owners(&base, &head, &evidence, REPOSITORY);
    assert!(result.is_err(), "stale approval: {result:?}");
}

#[test]
fn newly_added_owner_cannot_self_approve() {
    let (base, _, mut evidence) = fixture();
    let tree = MemoryTree::owned().edit("core/package.toml", "@synthetic/knowledge", "new-owner");
    let head = snapshot(HEAD, &tree);
    evidence.approvals[0].actor = "new-owner".into();
    evidence.approvals[0].files = head
        .files()
        .iter()
        .map(|(path, digest)| ApprovalFile {
            path: path.clone(),
            digest: Some(digest.clone()),
        })
        .collect();
    assert!(check_owners(&base, &head, &evidence, REPOSITORY).is_err());
    evidence.approvals[0].actor = "owner".into();
    assert_eq!(check_owners(&base, &head, &evidence, REPOSITORY), Ok(()));
}

#[test]
fn changed_digest_dismissed_and_missing_review_refuse() {
    let (base, head, evidence) = fixture();
    for mode in ["digest", "dismissed", "missing", "team-actor", "delegate"] {
        let mut evidence = evidence.clone();
        match mode {
            "digest" => evidence.approvals[0]
                .files
                .iter_mut()
                .for_each(|file| file.digest = None),
            "dismissed" => evidence.approvals[0].approved = false,
            "missing" => evidence.approvals.clear(),
            "team-actor" => evidence.approvals[0].actor = "@synthetic/knowledge".into(),
            _ => evidence.approvals[0].actor = "reader".into(),
        }
        assert!(
            check_owners(&base, &head, &evidence, REPOSITORY).is_err(),
            "{mode}"
        );
    }
}

#[test]
fn evidence_repository_revisions_and_duplicates_refuse() {
    let (base, head, evidence) = fixture();
    for mode in ["schema", "repository", "base", "head", "principal", "file"] {
        let mut evidence = evidence.clone();
        match mode {
            "schema" => evidence.schema = "maestro-owner-evidence/2".into(),
            "repository" => evidence.repository = "other/repository".into(),
            "base" => evidence.base = HEAD.into(),
            "head" => evidence.head = BASE.into(),
            "principal" => evidence.principals.push(user("@OWNER")),
            _ => {
                let file = evidence.approvals[0].files[0].clone();
                evidence.approvals[0].files.push(file);
            }
        }
        assert!(
            check_owners(&base, &head, &evidence, REPOSITORY).is_err(),
            "{mode}"
        );
    }
}
