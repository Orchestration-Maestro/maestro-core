use super::{load, load::BINDING};
use crate::failure::Failure;
use maestro_kernel::artifact::Digest;
use std::{
    fs,
    path::{Path, PathBuf},
};

const TABLE: &str = r#"{"schema": "maestro-source-classes/1", "rules": [
    {"host": "docs.example.org", "class": "official_docs", "label": "Docs"}
]}"#;

/// A fresh configuration directory for one test.
fn config() -> PathBuf {
    maestro_test_scratch::scratch_directory().unwrap()
}

/// Binds `source_classes` to `table` in `directory`, written when given.
fn bind(directory: &Path, table: Option<&str>) {
    let path = directory.join("table.json");
    if let Some(table) = table {
        fs::write(&path, table).unwrap();
    }
    fs::write(
        directory.join("bindings.toml"),
        format!("{BINDING} = '{}'\n", path.display()),
    )
    .unwrap();
}

#[test]
fn nothing_bound_means_no_classifier() {
    let directory = config();
    assert!(load(&directory).unwrap().is_none());
    fs::write(
        directory.join("bindings.toml"),
        format!("corpus_root = '{}'\n", directory.display()),
    )
    .unwrap();
    assert!(load(&directory).unwrap().is_none());
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_bound_table_loads_with_its_digest() {
    let directory = config();
    bind(&directory, Some(TABLE));
    let table = load(&directory).unwrap().unwrap();
    assert_eq!(table.digest(), &Digest::of(TABLE.as_bytes()));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn an_invalid_bindings_file_is_its_own_refusal() {
    let directory = config();
    fs::write(
        directory.join("bindings.toml"),
        "corpus_root = 'relative'\n",
    )
    .unwrap();
    assert!(matches!(load(&directory), Err(Failure::Refused(_))));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn a_missing_or_invalid_table_is_refused() {
    let directory = config();
    bind(&directory, None);
    assert!(matches!(load(&directory), Err(Failure::Refused(_))));
    bind(&directory, Some("{}"));
    assert!(matches!(load(&directory), Err(Failure::Refused(_))));
    fs::remove_dir_all(directory).unwrap();
}
