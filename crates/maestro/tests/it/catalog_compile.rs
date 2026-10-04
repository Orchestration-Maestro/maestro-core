//! `catalog compile`: determinism and exclusive publication through the process.

use super::{
    catalog_check::valid_catalog,
    support::{Ended, Home},
};
use std::{fs, path::Path};

/// Synthetic full release revision.
const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

/// Invoke the public CLI with a disposable output path.
fn compile(home: &Home, root: &Path, output: &Path, commit: &str) -> Ended {
    home.run(&[
        "--json",
        "catalog",
        "compile",
        "--catalog-dir",
        root.to_str().unwrap(),
        "--output",
        output.to_str().unwrap(),
        "--source-commit",
        commit,
    ])
}

#[test]
fn catalog_compile_compares_two_produced_bundle_digests() {
    let home = Home::bare();
    let root = valid_catalog(&home);
    let first = home.root().join("first.tar");
    let second = home.root().join("second.tar");
    let first_result = compile(&home, &root, &first, COMMIT);
    assert_eq!(first_result.code, Some(0), "{first_result:?}");
    let second_result = compile(&home, &root, &second, COMMIT);
    assert_eq!(second_result.code, Some(0), "{second_result:?}");
    assert_eq!(
        first_result.json()["digest"],
        second_result.json()["digest"]
    );
    assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
    assert_eq!(
        first_result.json()["schema"],
        "maestro-cli/catalog-compile/1"
    );
    assert_eq!(first_result.json()["status"], "compiled");
    assert!(fs::read_dir(home.root()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".maestro-bundle-")
    }));
    let before = fs::read(&first).unwrap();
    let refused = compile(&home, &root, &first, COMMIT);
    assert_eq!(refused.code, Some(1), "{refused:?}");
    assert_eq!(fs::read(&first).unwrap(), before);
}

#[test]
fn catalog_compile_refuses_before_output_creation() {
    let home = Home::bare();
    let root = valid_catalog(&home);
    let output = home.root().join("bundle.tar");
    let bad = compile(&home, &root, &output, "abc");
    assert_eq!(bad.code, Some(2), "{bad:?}");
    assert!(bad.stderr.contains("source_commit"));
    assert!(!output.exists());
    fs::write(root.join("package.toml"), "unknown = true").unwrap();
    let bad = compile(&home, &root, &output, COMMIT);
    assert_eq!(bad.code, Some(2), "{bad:?}");
    assert!(!output.exists());
}

#[test]
fn catalog_compile_reports_missing_source_and_output_parent() {
    let home = Home::bare();
    let root = valid_catalog(&home);
    let output = home.root().join("missing/bundle.tar");
    assert_eq!(compile(&home, &root, &output, COMMIT).code, Some(1));
    let absent = home.root().join("absent");
    assert_eq!(compile(&home, &absent, &output, COMMIT).code, Some(1));
    assert!(!output.exists());
}
