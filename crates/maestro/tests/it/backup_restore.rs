//! Backup and restore: online copies keep the database, artifacts and leased
//! work together; invalid backup input is refused before the target changes.

use super::support::Home;
use maestro_kernel::artifact::Digest;
use rusqlite::{Connection, OpenFlags, types::Value};
use serde_json::{Value as Json, json};
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

/// One backup mutation used in a refusal test.
type MutationCase = (&'static str, fn(&Path));

/// Writes a backup below `home` and checks the command's success response.
pub(super) fn backup(home: &Home, name: &str) -> PathBuf {
    let path = home.root().join(name);
    let argument = path.display().to_string();
    let result = home.run(&["--json", "backup", "--to", &argument]);
    assert_eq!(
        (result.code, result.stderr.as_str()),
        (Some(0), ""),
        "{result:?}"
    );
    assert_eq!(result.json()["schema"], "maestro-cli/backup/1");
    assert!(
        result
            .stdout
            .starts_with("{\"schema\":\"maestro-cli/backup/1\"")
    );
    path
}

/// Restores `backup` in `home` and checks a successful response's schema.
pub(super) fn restore(home: &Home, backup: &Path) -> super::support::Ended {
    let argument = backup.display().to_string();
    let ended = home.run(&["--json", "restore", "--from", &argument]);
    if ended.code == Some(0) {
        assert!(
            ended
                .stdout
                .starts_with("{\"schema\":\"maestro-cli/restore/1\"")
        );
    }
    ended
}

fn sqlite_contents(path: &Path) -> (Vec<String>, BTreeMap<String, Vec<Vec<Value>>>) {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let mut statement = connection
        .prepare(
            "SELECT name FROM sqlite_schema
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .unwrap();
    let tables: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap();
    drop(statement);
    let mut contents = BTreeMap::new();
    for table in &tables {
        let sql = format!("SELECT * FROM \"{}\"", table.replace('"', "\"\""));
        let mut statement = connection.prepare(&sql).unwrap();
        let columns = statement.column_count();
        let mut rows: Vec<Vec<Value>> = statement
            .query_map([], |row| {
                (0..columns)
                    .map(|column| row.get(column))
                    .collect::<rusqlite::Result<_>>()
            })
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        rows.sort_by_key(|row| format!("{row:?}"));
        contents.insert(table.clone(), rows);
    }
    (tables, contents)
}

/// Reads a backup manifest as JSON.
pub(super) fn manifest(path: &Path) -> Json {
    serde_json::from_slice(&fs::read(path.join("manifest.json")).unwrap()).unwrap()
}

/// Writes a JSON manifest to a backup directory.
pub(super) fn write_manifest(path: &Path, manifest: &Json) {
    fs::write(
        path.join("manifest.json"),
        serde_json::to_vec_pretty(manifest).unwrap(),
    )
    .unwrap();
}

/// Returns the first artifact path in a nonempty manifest.
pub(super) fn first_artifact(manifest: &Json) -> &str {
    manifest["artifacts"][0]["path"].as_str().unwrap()
}

/// The migrations the kernel database at `path` applied, by name, from the
/// first.
fn applied_migrations(path: &Path) -> Vec<String> {
    let names: Vec<String> = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .unwrap()
        .prepare("SELECT name FROM migrations ORDER BY name")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(names.first().map(String::as_str), Some("0001_artifacts"));
    names
}

/// Asserts restore refuses `backup` without installing kernel state.
pub(super) fn assert_refused_without_kernel(home: &Home, backup: &Path) {
    let ended = restore(home, backup);
    assert_eq!(ended.code, Some(2), "{ended:?}");
    assert_eq!(ended.stdout, "", "{ended:?}");
    assert!(!home.data().join("kernel.sqlite3").exists());
    assert!(!home.data().join("artifacts").exists());
}

#[test]
fn backup_has_the_database_rows_and_each_recorded_artifact() {
    let home = Home::new();
    home.add_synthetic();
    let source_database = home.data().join("kernel.sqlite3");
    let backup = backup(&home, "backup");
    let manifest = manifest(&backup);
    let database = backup.join("kernel.sqlite3");
    assert_eq!(manifest["schema"], "maestro-backup/1");
    let created_at = manifest["created_at"].as_str().unwrap();
    assert_eq!(created_at.len(), 24, "RFC 3339 UTC milliseconds");
    assert!(created_at.ends_with('Z'));
    let mut date = created_at.split('T').next().unwrap().split('-');
    let year = date.next().unwrap().parse::<u32>().unwrap();
    let month = date.next().unwrap().parse::<u32>().unwrap();
    let day = date.next().unwrap().parse::<u32>().unwrap();
    assert!(year >= 2000 && (1..=12).contains(&month) && (1..=31).contains(&day));
    assert_eq!(manifest["maestro_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        manifest["migrations"],
        json!(applied_migrations(&source_database))
    );
    assert_eq!(
        manifest["database"]["sha256"],
        Digest::of(&fs::read(&database).unwrap()).as_str()
    );
    assert_eq!(
        manifest["database"]["size"],
        fs::metadata(&database).unwrap().len()
    );
    assert_eq!(
        sqlite_contents(&source_database),
        sqlite_contents(&database),
        "all tables and rows match"
    );
    let artifacts = manifest["artifacts"].as_array().unwrap();
    let recorded: i64 =
        Connection::open_with_flags(&source_database, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap()
            .query_row("SELECT count(*) FROM artifacts", [], |row| row.get(0))
            .unwrap();
    assert_eq!(artifacts.len(), usize::try_from(recorded).unwrap());
    for artifact in artifacts {
        let path = backup.join(artifact["path"].as_str().unwrap());
        let bytes = fs::read(&path).unwrap();
        assert_eq!(artifact["sha256"], Digest::of(&bytes).as_str());
        assert_eq!(artifact["size"], bytes.len());
    }
    assert!(backup.join("artifacts").is_dir());
}

#[test]
fn backup_refuses_a_nonempty_destination_and_accepts_an_empty_one() {
    let home = Home::new();
    home.add_synthetic();
    let destination = home.root().join("occupied-backup");
    fs::create_dir(&destination).unwrap();
    let marker = destination.join("keep");
    fs::write(&marker, b"leave this untouched").unwrap();
    let argument = destination.display().to_string();
    let refused = home.run(&["--json", "backup", "--to", &argument]);
    assert_eq!(refused.code, Some(2), "{refused:?}");
    assert_eq!(fs::read(&marker).unwrap(), b"leave this untouched");

    let empty = home.root().join("empty-backup");
    fs::create_dir(&empty).unwrap();
    let argument = empty.display().to_string();
    let accepted = home.run(&["--json", "backup", "--to", &argument]);
    assert_eq!(accepted.code, Some(0), "{accepted:?}");
    assert_eq!(accepted.json()["schema"], "maestro-cli/backup/1");
}

#[test]
fn backup_never_creates_or_migrates_the_source_database() {
    let empty = Home::new();
    let destination = empty.root().join("empty-kernel-backup");
    let destination_arg = destination.display().to_string();
    let failed = empty.run(&["--json", "backup", "--to", &destination_arg]);
    assert_eq!(failed.code, Some(1), "{failed:?}");
    assert!(!empty.data().join("kernel.sqlite3").exists());
    assert!(!destination.exists());

    let home = Home::new();
    home.add_synthetic();
    let database_path = home.data().join("kernel.sqlite3");
    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute("DELETE FROM migrations WHERE name = '0007_chunk_sets'", [])
        .unwrap();
    drop(connection);
    let destination = backup(&home, "older-kernel-backup");
    for path in [&database_path, &destination.join("kernel.sqlite3")] {
        let connection =
            Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let missing: i64 = connection
            .query_row(
                "SELECT count(*) FROM migrations WHERE name = '0007_chunk_sets'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(missing, 0, "backup did not migrate {path:?}");
    }
}

#[test]
fn backup_restores_a_running_job_and_its_progress_for_takeover() {
    use maestro_kernel::{
        job::NewJob,
        scope::{Config, LOCAL},
    };
    use serde_json::json;

    let source = Home::new();
    source.add_synthetic();
    let database = source.database();
    let scope = "workspace/default/collection/synthetic".parse().unwrap();
    let inputs = json!({ "backup": true });
    let job = database
        .submit_job(
            &NewJob {
                kind: "knowledge.import",
                inputs: &inputs,
                scope: &scope,
                resource: None,
            },
            SystemTime::now(),
        )
        .unwrap();
    let now = SystemTime::now();
    let term = Duration::from_secs(60);
    let mut lease = database.take_job(job.id, "worker-one", now, term).unwrap();
    database
        .progress(&mut lease, now, term, &json!({ "step": 3 }))
        .unwrap();
    let backup = backup(&source, "job-backup");

    let restored = Home::new();
    let ended = restore(&restored, &backup);
    assert_eq!(
        (ended.code, ended.stderr.as_str()),
        (Some(0), ""),
        "{ended:?}"
    );
    assert_eq!(ended.json()["schema"], "maestro-cli/restore/1");
    let restored_database = restored.database();
    restored_database
        .apply_config(&Config::load(&restored.config()).unwrap())
        .unwrap();
    let scopes = restored_database.visible(LOCAL).unwrap();
    let still_running = restored_database.job(&scopes, job.id).unwrap().unwrap();
    assert_eq!(still_running.state.to_string(), "running");
    assert_eq!(
        restored_database
            .last_progress(&scopes, job.id)
            .unwrap()
            .unwrap()
            .data,
        json!({ "step": 3 })
    );
    let took_over_at = now + term + Duration::from_secs(1);
    let mut successor = restored_database
        .take_job(job.id, "worker-two", took_over_at, term)
        .unwrap();
    restored_database
        .progress(&mut successor, took_over_at, term, &json!({ "step": 4 }))
        .unwrap();
    assert_eq!(
        restored_database
            .last_progress(&scopes, job.id)
            .unwrap()
            .unwrap()
            .data,
        json!({ "step": 4 }),
        "the resumed work follows the last journaled step"
    );
}

#[test]
fn backup_and_restore_keep_knowledge_status_identical() {
    let source = Home::new();
    source.add_synthetic();
    let expected = source.run(&["--json", "knowledge", "status", "--collection", "synthetic"]);
    assert_eq!(expected.code, Some(0), "{expected:?}");
    let backup = backup(&source, "status-backup");
    let target = Home::new();
    let legacy_files = [
        ("ledger.sqlite3", b"leave ledger untouched".as_slice()),
        ("maestro.sock", b"leave socket untouched".as_slice()),
        ("supervisor.lock", b"leave lock untouched".as_slice()),
    ];
    for (name, contents) in legacy_files {
        fs::write(target.data().join(name), contents).unwrap();
    }
    let material = target.data().join("material");
    fs::create_dir(&material).unwrap();
    fs::write(material.join("keep"), b"leave material untouched").unwrap();
    let restored = restore(&target, &backup);
    assert_eq!(restored.code, Some(0), "{restored:?}");
    for (name, contents) in legacy_files {
        assert_eq!(fs::read(target.data().join(name)).unwrap(), contents);
    }
    assert_eq!(
        fs::read(material.join("keep")).unwrap(),
        b"leave material untouched"
    );
    let actual = target.run(&["--json", "knowledge", "status", "--collection", "synthetic"]);
    assert_eq!(actual.code, Some(0), "{actual:?}");
    assert_eq!(actual.json(), expected.json());
}

#[test]
fn restore_refuses_invalid_paths_unlisted_or_missing_artifacts_and_hash_mismatches() {
    let source = Home::new();
    source.add_synthetic();
    let original = backup(&source, "invalid-backup");
    let cases: [MutationCase; 10] = [
        ("unsupported schema", |root| {
            let mut value = manifest(root);
            value["schema"] = json!("maestro-backup/99");
            write_manifest(root, &value);
        }),
        ("empty creation time", |root| {
            let mut value = manifest(root);
            value["created_at"] = json!("");
            write_manifest(root, &value);
        }),
        ("empty maestro version", |root| {
            let mut value = manifest(root);
            value["maestro_version"] = json!("");
            write_manifest(root, &value);
        }),
        ("duplicate artifact", |root| {
            let mut value = manifest(root);
            let duplicate = value["artifacts"][0].clone();
            value["artifacts"].as_array_mut().unwrap().push(duplicate);
            write_manifest(root, &value);
        }),
        ("parent path", |root| {
            let mut value = manifest(root);
            value["artifacts"][0]["path"] = json!("../outside");
            write_manifest(root, &value);
        }),
        ("absolute path", |root| {
            let mut value = manifest(root);
            value["artifacts"][0]["path"] = json!(env::temp_dir().display().to_string());
            write_manifest(root, &value);
        }),
        ("unlisted file", |root| {
            fs::write(root.join("unlisted"), b"extra").unwrap();
        }),
        ("missing file", |root| {
            fs::remove_file(root.join(first_artifact(&manifest(root)))).unwrap();
        }),
        ("digest mismatch", |root| {
            let mut value = manifest(root);
            let path = root.join(first_artifact(&value));
            let mut bytes = fs::read(&path).unwrap();
            bytes.push(1);
            fs::write(&path, &bytes).unwrap();
            value["artifacts"][0]["size"] = json!(bytes.len());
            write_manifest(root, &value);
        }),
        ("size mismatch", |root| {
            let mut value = manifest(root);
            value["artifacts"][0]["size"] = json!(999_999);
            write_manifest(root, &value);
        }),
    ];
    for (name, mutate) in cases {
        let damaged = source.root().join(format!("damaged-{name}"));
        fs::create_dir(&damaged).unwrap();
        copy_tree(&original, &damaged);
        mutate(&damaged);
        let target = Home::new();
        assert_refused_without_kernel(&target, &damaged);
    }
}

#[test]
fn restore_rejects_a_non_directory_backup_root_before_opening_children() {
    let target = Home::new();
    let backup_file = target.root().join("backup-file");
    fs::write(&backup_file, b"not a backup directory").unwrap();

    let ended = restore(&target, &backup_file);
    assert_eq!(ended.code, Some(2), "{ended:?}");
    assert!(ended.stderr.contains("is not a directory"), "{ended:?}");
}

#[test]
fn restore_checks_file_integrity_before_touching_the_target_directory() {
    let source = Home::new();
    source.add_synthetic();
    let backup = backup(&source, "corrupt-before-stage-backup");
    let artifact = backup.join(first_artifact(&manifest(&backup)));
    let mut bytes = fs::read(&artifact).unwrap();
    assert!(!bytes.is_empty());
    bytes[0] ^= 1;
    fs::write(&artifact, &bytes).unwrap();

    let target = Home::new();
    let data_root = target.root().join("data");
    fs::remove_dir_all(&data_root).unwrap();
    fs::write(&data_root, b"block restore writes").unwrap();
    let ended = restore(&target, &backup);
    assert_eq!(ended.code, Some(2), "{ended:?}");
    assert_eq!(fs::read(data_root).unwrap(), b"block restore writes");
}

fn copy_tree(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let destination = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            fs::create_dir(&destination).unwrap();
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

#[cfg(unix)]
#[test]
fn restore_refuses_symbolic_links() {
    use std::os::unix::fs::symlink;

    let source = Home::new();
    source.add_synthetic();
    let backup = backup(&source, "symlink-backup");
    let manifest = manifest(&backup);
    let artifact = backup.join(first_artifact(&manifest));
    let outside = source.root().join("outside-artifact");
    fs::rename(&artifact, &outside).unwrap();
    symlink(&outside, &artifact).unwrap();
    assert_refused_without_kernel(&Home::new(), &backup);
}

#[test]
fn restore_refuses_hard_links() {
    let source = Home::new();
    source.add_synthetic();
    let backup = backup(&source, "hardlink-backup");
    let manifest = manifest(&backup);
    let artifact = backup.join(first_artifact(&manifest));
    let outside = source.root().join("outside-hardlink");
    fs::rename(&artifact, &outside).unwrap();
    fs::hard_link(&outside, &artifact).unwrap();
    assert_refused_without_kernel(&Home::new(), &backup);
}

#[test]
fn restore_refuses_unknown_migrations() {
    let source = Home::new();
    source.add_synthetic();
    let unknown_backup = backup(&source, "unknown-migration-backup");
    let database_path = unknown_backup.join("kernel.sqlite3");
    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute(
            "INSERT INTO migrations (name, applied_at) VALUES ('9999_future', 'now')",
            [],
        )
        .unwrap();
    drop(connection);
    let mut value = manifest(&unknown_backup);
    value["migrations"]
        .as_array_mut()
        .unwrap()
        .push(json!("9999_future"));
    let bytes = fs::read(&database_path).unwrap();
    value["database"]["sha256"] = json!(Digest::of(&bytes).as_str());
    value["database"]["size"] = json!(bytes.len());
    write_manifest(&unknown_backup, &value);
    assert_refused_without_kernel(&Home::new(), &unknown_backup);
}
