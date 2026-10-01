//! Git administration is outside the filesystem source view, not a dotfile filter.

use super::{area_support::scoped, support::check_by};
use crate::{
    limits::Limits,
    source::{Directory, Registry, SourceTree, walk::walk},
};
use std::{fs, path::PathBuf};

/// One real filesystem fixture, removed on drop.
struct Scratch(PathBuf);

impl Scratch {
    /// Write a valid resource beside root Git administration.
    fn new() -> Self {
        let root = maestro_test_scratch::scratch_directory().unwrap();
        let text = concat!(
            "term='evidence'\n[metadata]\n",
            "schema='maestro-source/1'\nowner='@synthetic/knowledge'\n",
            "maturity='authored'\nrows=['chat.M036 objects']\nworkflows=['ctm-question']\n",
        );
        fs::write(root.join("package.toml"), text).unwrap();
        Self(root)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(fs::remove_dir_all(&self.0));
    }
}

/// Check and discover exactly the one published primary under its exact budgets.
fn accepts(root: &Scratch) {
    let tree = Directory::new(&root.0);
    let mut registry = Registry::default();
    registry.register(scoped("", &["common"])).unwrap();
    let mut limits = Limits::PRODUCTION;
    limits.archive_entries = 1;
    limits.archive_total_bytes = fs::metadata(root.0.join("package.toml")).unwrap().len();
    let production = check_by(&tree, &registry, &Limits::PRODUCTION);
    assert!(production.is_ok(), "{production:?}");
    let result = walk(&tree, &registry, &limits);
    assert!(result.is_ok(), "{result:?}");
    let found = result.unwrap();
    assert!(found.diagnostics.is_empty(), "{:?}", found.diagnostics);
    assert_eq!(found.units.len(), 1);
    assert_eq!(found.units[0].path, "package.toml");
    assert_eq!(
        check_by(&tree, &registry, &limits).unwrap().resources.len(),
        1
    );
    assert_eq!(tree.list_bounded("", 1).unwrap()[0].name, "package.toml");
}

#[test]
fn root_git_large_pack_is_outside_source_and_budgets() {
    let root = Scratch::new();
    fs::create_dir_all(root.0.join(".git/objects/pack")).unwrap();
    fs::write(
        root.0.join(".git/objects/pack/large.pack"),
        vec![0; 1_048_577],
    )
    .unwrap();
    accepts(&root);
}

#[test]
fn root_git_file_is_outside_source_and_budgets() {
    let root = Scratch::new();
    fs::write(root.0.join(".git"), "gitdir: elsewhere").unwrap();
    accepts(&root);
}

#[test]
fn nested_git_is_unregistered_even_when_empty() {
    let root = Scratch::new();
    fs::create_dir_all(root.0.join("docs/.git")).unwrap();
    let mut registry = Registry::default();
    registry.register(scoped("", &["common"])).unwrap();
    let tree = Directory::new(&root.0);
    let found = walk(&tree, &registry, &Limits::PRODUCTION).unwrap();
    assert!(
        found
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path == "docs/.git")
    );
    assert!(check_by(&tree, &registry, &Limits::PRODUCTION).is_err());
}

#[test]
fn github_and_other_dot_entries_remain_presented() {
    let root = Scratch::new();
    fs::create_dir(root.0.join(".github")).unwrap();
    fs::write(root.0.join(".github/valid.toml"), "data").unwrap();
    fs::write(root.0.join(".other"), "data").unwrap();
    let tree = Directory::new(&root.0);
    let mut registry = Registry::default();
    registry.register(scoped(".github", &["root"])).unwrap();
    let found = walk(&tree, &registry, &Limits::PRODUCTION).unwrap();
    assert_eq!(found.units.len(), 1);
    assert_eq!(found.units[0].path, ".github/valid.toml");
    assert!(
        found
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.path == ".other")
    );
    assert_eq!(
        tree.list("")
            .unwrap()
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>(),
        [".github", ".other", "package.toml"]
    );
}

#[test]
fn legacy_check_refuses_nested_git_administration() {
    use super::support::VALID;
    use crate::source::kinds::legacy as builtin;
    let root = Scratch::new();
    fs::remove_file(root.0.join("package.toml")).unwrap();
    for (file, text) in VALID {
        let path = root.0.join(file);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fs::create_dir_all(root.0.join("bootstrap/base/.git")).unwrap();
    let result = check_by(
        &Directory::new(&root.0),
        &builtin().unwrap(),
        &Limits::PRODUCTION,
    );
    assert!(result.is_err(), "{result:?}");
}
