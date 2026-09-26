//! The kernel's checks: its configuration files, its database, opened only
//! if it exists and checked whole, and its artifact tree, each failure with
//! its next action.

use super::{
    super::kernel::{artifacts_check, bindings_check, config_check, database_check},
    support::{Scratch, detail, failure},
};
use maestro_kernel::{
    scope::{LOCAL, Scope},
    store::Database,
};
use rusqlite::Connection;
use std::{
    fs::{self, OpenOptions},
    io::{Seek as _, SeekFrom, Write as _},
};

#[test]
fn valid_configuration_files_pass() {
    let scratch = Scratch::new();
    assert_eq!(detail(&config_check(&scratch.config()).0), "valid");
    assert_eq!(detail(&bindings_check(&scratch.config())), "valid");
    scratch.configure("config.toml", "[access]\nread = ['workspace/default']\n");
    scratch.configure("bindings.toml", "corpus_root = '/srv/corpus'\n");
    let (check, config) = config_check(&scratch.config());
    assert_eq!(detail(&check), "valid");
    assert!(config.is_some());
    assert_eq!(
        check.target,
        scratch.config().join("config.toml").display().to_string()
    );
    let check = bindings_check(&scratch.config());
    assert_eq!(detail(&check), "valid");
    assert_eq!(
        check.target,
        scratch.config().join("bindings.toml").display().to_string()
    );
}

#[test]
fn a_refused_configuration_file_fails_naming_it_and_how_to_fix_it() {
    let scratch = Scratch::new();
    scratch.configure("config.toml", "[access]\nwrite = ['workspace/default']\n");
    scratch.configure("bindings.toml", "corpus_root = 'relative'\n");
    let (check, config) = config_check(&scratch.config());
    assert!(config.is_none());
    let (problem, next) = failure(&check);
    assert!(problem.contains("access.write"), "{problem}");
    assert!(
        next.contains("config.toml") && next.contains("read"),
        "{next}"
    );
    let check = bindings_check(&scratch.config());
    let (problem, next) = failure(&check);
    assert!(problem.contains("corpus_root"), "{problem}");
    assert!(
        next.contains("bindings.toml") && next.contains("absolute"),
        "{next}"
    );
}

#[test]
fn a_missing_database_fails_and_is_never_created() {
    let scratch = Scratch::new();
    let (check, opened) = database_check(&scratch.data(), None);
    assert!(opened.is_none());
    let database = scratch.data().join("kernel.sqlite3");
    assert_eq!(check.target, database.display().to_string());
    let (problem, next) = failure(&check);
    assert!(problem.contains("no kernel database"), "{problem}");
    assert!(next.contains("maestro knowledge collection add"), "{next}");
    assert!(!database.exists(), "a check never creates the database");
    let check = artifacts_check(&scratch.data(), opened.as_ref());
    let (problem, next) = failure(&check);
    assert!(problem.contains("kernel database"), "{problem}");
    assert!(next.contains("database"), "{next}");
}

#[test]
fn an_intact_database_passes_and_applies_the_grants_of_config_toml() {
    let scratch = Scratch::new();
    drop(Database::open_in(&scratch.data()).unwrap());
    scratch.configure("config.toml", "[access]\nread = ['workspace/default']\n");
    let config = config_check(&scratch.config()).1.unwrap();
    let (check, opened) = database_check(&scratch.data(), Some(&config));
    assert_eq!(detail(&check), "intact");
    let opened = opened.unwrap();
    let granted: Vec<&str> = opened
        .scopes
        .as_ref()
        .unwrap()
        .granted()
        .map(Scope::as_str)
        .collect();
    assert_eq!(granted, ["workspace/default"]);
    let (_, unconfigured) = database_check(&scratch.data(), None);
    assert!(
        unconfigured.unwrap().scopes.is_none(),
        "no grants without config.toml"
    );
    let reread = Database::open_in(&scratch.data()).unwrap();
    assert_eq!(reread.visible(LOCAL).unwrap().granted().count(), 1);
}

#[test]
fn a_damaged_database_fails_with_what_sqlite_found() {
    let scratch = Scratch::new();
    drop(Database::open_in(&scratch.data()).unwrap());
    // The header's count of free pages, 5, where the file has none.
    let mut file = OpenOptions::new()
        .write(true)
        .open(scratch.data().join("kernel.sqlite3"))
        .unwrap();
    file.seek(SeekFrom::Start(36)).unwrap();
    file.write_all(&5_u32.to_be_bytes()).unwrap();
    drop(file);
    let (check, _) = database_check(&scratch.data(), None);
    let (problem, next) = failure(&check);
    assert!(
        problem.contains("Freelist: size is 0 but should be 5"),
        "{problem}"
    );
    assert!(next.contains("backup"), "{next}");
}

#[test]
fn a_database_that_cannot_open_fails_saying_why() {
    let scratch = Scratch::new();
    fs::create_dir(scratch.data().join("kernel.sqlite3")).unwrap();
    let (check, opened) = database_check(&scratch.data(), None);
    assert!(opened.is_none());
    let (problem, next) = failure(&check);
    assert!(problem.contains("cannot be opened"), "{problem}");
    assert!(next.contains("permissions"), "{next}");
}

#[test]
fn a_database_a_newer_maestro_migrated_fails_and_names_the_update() {
    let scratch = Scratch::new();
    drop(Database::open_in(&scratch.data()).unwrap());
    Connection::open(scratch.data().join("kernel.sqlite3"))
        .unwrap()
        .execute(
            "INSERT INTO migrations (name, applied_at) VALUES ('9999_future', 'now')",
            [],
        )
        .unwrap();
    let (check, opened) = database_check(&scratch.data(), None);
    assert!(opened.is_none());
    let (problem, next) = failure(&check);
    assert!(problem.contains("9999_future"), "{problem}");
    assert!(
        next.contains("newer maestro") && !next.contains("permissions"),
        "{next}"
    );
}

#[test]
fn a_database_that_lacks_a_migration_is_reported_and_never_migrated() {
    let scratch = Scratch::new();
    drop(Database::open_in(&scratch.data()).unwrap());
    let file = scratch.data().join("kernel.sqlite3");
    let outside = Connection::open(&file).unwrap();
    let last: String = outside
        .query_row("SELECT max(name) FROM migrations", [], |row| row.get(0))
        .unwrap();
    outside
        .execute("DELETE FROM migrations WHERE name = ?1", [&last])
        .unwrap();
    drop(outside);
    let (check, opened) = database_check(&scratch.data(), None);
    assert!(
        opened.is_none(),
        "a database that lacks a migration is not opened"
    );
    let (problem, next) = failure(&check);
    assert_eq!(
        problem,
        format!(
            "the kernel database lacks {last}, which this maestro applies when a command opens it"
        )
    );
    assert!(
        next.contains("any `maestro knowledge` command applies it")
            && next.contains("the maestro that last opened it"),
        "{next}"
    );
    let recorded: i64 = Connection::open(&file)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM migrations WHERE name = ?1",
            [&last],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(recorded, 0, "the check did not apply {last}");
}

#[test]
fn the_artifact_tree_passes_intact_and_fails_naming_what_is_missing_or_damaged() {
    let scratch = Scratch::new();
    let database = Database::open_in(&scratch.data()).unwrap();
    let kept = database.put(b"kept", "text/plain").unwrap();
    let lost = database.put(b"lost", "text/plain").unwrap();
    drop(database);
    let (_, opened) = database_check(&scratch.data(), None);
    let check = artifacts_check(&scratch.data(), opened.as_ref());
    assert_eq!(
        check.target,
        scratch.data().join("artifacts").display().to_string()
    );
    assert_eq!(detail(&check), "2 artifacts, each intact");
    let hex = lost.as_str();
    fs::remove_file(
        scratch
            .data()
            .join("artifacts/sha256")
            .join(&hex[..2])
            .join(&hex[2..4])
            .join(hex),
    )
    .unwrap();
    let check = artifacts_check(&scratch.data(), opened.as_ref());
    let (problem, next) = failure(&check);
    assert_eq!(
        problem,
        format!("1 of 2 artifacts missing and 0 damaged, the first sha256:{hex}")
    );
    assert!(next.contains("backup"), "{next}");
    assert!(!problem.contains(kept.as_str()));
}
