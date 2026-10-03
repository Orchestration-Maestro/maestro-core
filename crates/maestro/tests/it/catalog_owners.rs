//! Trusted evidence is a separate input; the command performs no lookup or write.

use super::support::{Ended, Home};
use maestro_catalog::limits::Limits;
use maestro_kernel::artifact::Digest;
use serde_json::{Value, json};
use std::{fs, path::PathBuf};

/// Exact commits selected by the workflow, never catalog metadata.
const BASE: &str = "1111111111111111111111111111111111111111";
/// Proposed commit.
const HEAD: &str = "2222222222222222222222222222222222222222";

/// Separate synthetic base/head trees and a trusted evidence file outside both.
fn fixture(home: &Home) -> (PathBuf, PathBuf, PathBuf, Value) {
    let base = home.root().join("base");
    let head = home.root().join("head");
    fs::create_dir_all(&base).unwrap();
    fs::create_dir_all(&head).unwrap();
    let source = include_str!("../../../../tests/fixtures/catalog/source/package.toml")
        .replace("@synthetic/knowledge", "owner");
    fs::write(base.join("package.toml"), &source).unwrap();
    let changed = source.replace("description =", "maintainers = [\"reader\"]\ndescription =");
    fs::write(head.join("package.toml"), &changed).unwrap();
    let evidence = json!({
        "schema": "maestro-owner-evidence/1", "repository": "synthetic/manifests",
        "base": BASE, "head": HEAD,
        "principals": [
            {"principal": "owner", "exists": true, "repository_access": true, "members": []},
            {"principal": "reader", "exists": true, "repository_access": true, "members": []}
        ],
        "approvals": [{"actor": "owner", "head": HEAD, "approved": true,
            "files": [{"path": "package.toml", "digest": Digest::of(changed.as_bytes()).as_str()}]}]
    });
    (base, head, home.root().join("evidence.json"), evidence)
}

/// Invoke the real binary with explicit trusted repository/revision bindings.
fn run(home: &Home, (base, head, path): (&PathBuf, &PathBuf, &PathBuf)) -> Ended {
    home.run(&[
        "--json",
        "catalog",
        "owners",
        "--check-identities",
        "--base-dir",
        base.to_str().unwrap(),
        "--catalog-dir",
        head.to_str().unwrap(),
        "--evidence",
        path.to_str().unwrap(),
        "--repository",
        "synthetic/manifests",
        "--base-revision",
        BASE,
        "--head-revision",
        HEAD,
    ])
}

#[test]
fn catalog_owners_accepts_external_evidence_without_writing() {
    let home = Home::bare();
    let (base, head, path, evidence) = fixture(&home);
    fs::write(&path, serde_json::to_vec(&evidence).unwrap()).unwrap();
    let before = fs::read(head.join("package.toml")).unwrap();
    let result = run(&home, (&base, &head, &path));
    assert_eq!(result.code, Some(0), "{result:?}");
    assert_eq!(result.json()["schema"], "maestro-cli/catalog-owners/1");
    assert_eq!(result.json()["status"], "passed");
    assert_eq!(fs::read(head.join("package.toml")).unwrap(), before);
    assert_eq!(fs::read_dir(&head).unwrap().count(), 1);
}

#[test]
fn catalog_owners_refuses_stale_access_self_approval_and_digest() {
    let home = Home::bare();
    let (base, head, path, evidence) = fixture(&home);
    for mode in ["stale", "access", "self", "digest", "unknown"] {
        let mut evidence = evidence.clone();
        match mode {
            "stale" => evidence["approvals"][0]["head"] = json!(BASE),
            "access" => evidence["principals"][0]["repository_access"] = json!(false),
            "self" => evidence["approvals"][0]["actor"] = json!("reader"),
            "digest" => evidence["approvals"][0]["files"][0]["digest"] = json!("a".repeat(64)),
            _ => evidence["principals"][0]["exists"] = json!(false),
        }
        fs::write(&path, serde_json::to_vec(&evidence).unwrap()).unwrap();
        let result = run(&home, (&base, &head, &path));
        assert_eq!(result.code, Some(2), "{mode}: {result:?}");
        assert!(
            result.stderr.contains("package.toml")
                || result.stderr.contains("owner evidence review actor"),
            "{result:?}"
        );
    }
}

#[test]
fn catalog_owners_missing_or_malformed_evidence_never_passes() {
    let home = Home::bare();
    let (base, head, path, _) = fixture(&home);
    let missing = run(&home, (&base, &head, &path));
    assert_eq!(missing.code, Some(1), "{missing:?}");
    for bytes in [
        b"[]".as_slice(),
        b"{\"schema\":\"x\",\"schema\":\"y\"}",
        b"{\"token\":\"not-a-credential\"}",
    ] {
        fs::write(&path, bytes).unwrap();
        let result = run(&home, (&base, &head, &path));
        assert_eq!(result.code, Some(2), "{result:?}");
        assert!(result.stderr.contains("evidence.json"), "{result:?}");
    }
}

#[test]
fn catalog_owners_evidence_byte_limit_is_inclusive() {
    let home = Home::bare();
    let (base, head, path, evidence) = fixture(&home);
    let limit = usize::try_from(Limits::PRODUCTION.source_file_bytes).unwrap();
    let mut bytes = serde_json::to_vec(&evidence).unwrap();
    bytes.resize(limit, b' ');
    fs::write(&path, &bytes).unwrap();
    let result = run(&home, (&base, &head, &path));
    assert_eq!(result.code, Some(0), "{result:?}");
    bytes.push(b' ');
    fs::write(&path, &bytes).unwrap();
    let result = run(&home, (&base, &head, &path));
    assert_eq!(result.code, Some(2), "{result:?}");
    assert!(result.stderr.contains("byte limit"), "{result:?}");
}

#[cfg(unix)]
#[test]
fn catalog_owners_evidence_symlink_never_reads_its_target() {
    use std::os::unix::fs::symlink;
    let home = Home::bare();
    let (base, head, path, evidence) = fixture(&home);
    let target = home.root().join("outside.json");
    fs::write(&target, serde_json::to_vec(&evidence).unwrap()).unwrap();
    symlink(&target, &path).unwrap();
    let result = run(&home, (&base, &head, &path));
    assert_eq!(result.code, Some(1), "{result:?}");
    assert!(result.stderr.contains("evidence.json"), "{result:?}");
}

#[test]
fn catalog_owners_reads_a_bare_relative_evidence_filename() {
    let home = Home::bare();
    let (base, head, path, evidence) = fixture(&home);
    fs::write(&path, serde_json::to_vec(&evidence).unwrap()).unwrap();
    let relative = PathBuf::from("evidence.json");
    let result = run(&home, (&base, &head, &relative));
    assert_eq!(result.code, Some(0), "{result:?}");
    fs::remove_file(&path).unwrap();
    let directory = home.root().join("nested");
    fs::create_dir(&directory).unwrap();
    let nested = directory.join("evidence.json");
    fs::write(&nested, serde_json::to_vec(&evidence).unwrap()).unwrap();
    let result = run(&home, (&base, &head, &nested));
    assert_eq!(result.code, Some(0), "{result:?}");
}
