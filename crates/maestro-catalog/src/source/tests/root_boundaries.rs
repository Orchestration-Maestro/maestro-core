//! Root occupancy and public filesystem spelling neighbours.
use super::{
    area_support::{discover, scoped},
    support::MemoryTree,
};
use crate::{
    limits::Limits,
    source::{KindDescriptor, Layout, Registry},
};
// Unix permits byte names; Windows uses Unicode names.
#[cfg(unix)]
use crate::source::{Directory, SourceTree};
#[cfg(unix)]
use std::fs;
fn root_descriptors() -> (KindDescriptor, KindDescriptor) {
    let files = scoped("", &["common"]);
    let mut single = files.clone();
    single.kind = "other".to_owned();
    single.layout = Layout::Single {
        file: "package.toml".to_owned(),
        name: "package".to_owned(),
    };
    (files, single)
}

fn root_registration_case(reverse: bool) {
    let (files, single) = root_descriptors();
    let (first, second) = if reverse {
        (single, files)
    } else {
        (files, single)
    };
    let mut registry = Registry::default();
    let first = registry.register(first);
    let second = registry.register(second);
    let found = discover(
        &MemoryTree::default().with("package.toml", "data"),
        &registry,
        &Limits::PRODUCTION,
    );
    let diagnostics: Vec<_> = found.diagnostics.iter().map(ToString::to_string).collect();
    assert!(
        first.is_err()
            || second
                .as_ref()
                .is_err_and(|error| error.to_string().contains("overlapping placement"))
            || !diagnostics.is_empty()
    );
}

#[test]
fn c30_review_probe_root_registration_a_then_b() {
    root_registration_case(false);
}
#[test]
fn c30_review_probe_root_registration_b_then_a() {
    root_registration_case(true);
}

#[test]
fn c30_review_probe_root_discovery() {
    let (files, single) = root_descriptors();
    let mut registry = Registry::default();
    let first = registry.register(files);
    let second = registry.register(single);
    let found = discover(
        &MemoryTree::default().with("package.toml", "data"),
        &registry,
        &Limits::PRODUCTION,
    );
    let diagnostics: Vec<_> = found.diagnostics.iter().map(ToString::to_string).collect();
    assert!(first.is_err() || second.is_err() || !diagnostics.is_empty());
}

#[test]
fn c30_review_probe_root_full_check() {
    let (files, single) = root_descriptors();
    assert_eq!(files.fields, single.fields);
    assert_eq!(files.hook, single.hook);
    let mut registry = Registry::default();
    let first = registry.register(files);
    let second = registry.register(single);
    // Confirmed existing fixture row: source/tests/extension.rs:30.
    let text = concat!(
        "term='evidence'\n[metadata]\n",
        "schema='maestro-source/2'\nowner='@synthetic/knowledge'\n",
        "maturity='authored'\nrows=['chat.M036 objects']\nworkflows=['ctm-question']\n",
    );
    let result = super::support::check_by(
        &MemoryTree::default().with("package.toml", text),
        &registry,
        &Limits::PRODUCTION,
    );
    assert!(first.is_err() || second.is_err() || result.is_err());
}

#[test]
fn c30_review_probe_root_single_non_overlap() {
    let (mut files, single) = root_descriptors();
    files.layout = Layout::Single {
        file: "other.toml".to_owned(),
        name: "other".to_owned(),
    };
    let mut registry = Registry::default();
    let first = registry.register(files);
    let second = registry.register(single);
    let tree = MemoryTree::default()
        .with("other.toml", "data")
        .with("package.toml", "data");
    let found = discover(&tree, &registry, &Limits::PRODUCTION);
    let paths: Vec<_> = found.units.iter().map(|unit| unit.path.as_str()).collect();
    let diagnostics: Vec<_> = found.diagnostics.iter().map(ToString::to_string).collect();
    assert!(
        first.is_ok()
            && second.is_ok()
            && paths == ["other.toml", "package.toml"]
            && diagnostics.is_empty()
    );
}

#[cfg(unix)]
#[test]
fn c30_review_probe_directory_lossy_name_order() {
    use crate::source::EntryKind;
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let scratch = maestro_test_scratch::scratch_directory().unwrap();
    fs::write(scratch.join(OsString::from_vec(vec![0x80])), b"").unwrap();
    fs::write(scratch.join("é"), b"").unwrap();
    let entries = Directory::new(&scratch).list("").unwrap();
    fs::remove_dir_all(scratch).unwrap();
    let names: Vec<_> = entries.iter().map(|entry| entry.name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
    assert_eq!(
        entries.iter().find(|entry| entry.name == "�").unwrap().kind,
        EntryKind::Unsupported
    );
}
