//! The `backup` and `restore` command handlers and backup writer.

use super::{
    filesystem::{
        copy_hash, create_destination, create_private_dir, create_private_dir_all,
        create_private_file, directory_problem, failed_io, hash_file, source_file, timestamp,
        write_private,
    },
    manifest::{
        BACKUP_SCHEMA, FileRecord, Manifest, artifact_manifest_path, native_path,
        recorded_migrations, validate_backup,
    },
    names::{ARTIFACTS, DATABASE, MANIFEST},
    restore::restore_files,
};
use crate::failure::Failure;
use maestro_kernel::{
    artifact::Digest,
    paths::{self, Environment},
};
use rusqlite::{Connection, OpenFlags, backup::Backup};
use serde::Serialize;
use std::{path::Path, process, time::Duration};

/// Schema of the JSON document printed by `backup`.
const BACKUP_OUTPUT_SCHEMA: &str = "maestro-cli/backup/1";
/// Schema of the JSON document printed by `restore`.
const RESTORE_OUTPUT_SCHEMA: &str = "maestro-cli/restore/1";

/// The JSON document printed by `backup`.
#[derive(Serialize)]
struct BackupOutput<'a> {
    /// The versioned output schema.
    schema: &'static str,
    /// The backup directory.
    to: &'a str,
}

/// The JSON document printed by `restore`.
#[derive(Serialize)]
struct RestoreOutput<'a> {
    /// The versioned output schema.
    schema: &'static str,
    /// The restored backup directory.
    from: &'a str,
    /// How many artifacts were restored.
    artifacts: usize,
}

/// Backs up the kernel named by the process environment, without creating or
/// migrating its database.
pub(in crate::cli) fn run_backup(
    output: super::super::output::Output,
    destination: &Path,
) -> Result<process::ExitCode, Failure> {
    let environment = Environment::current();
    let data = paths::data_dir(&environment).map_err(|error| Failure::failed_by(&error))?;
    if let Some(problem) = directory_problem(destination)? {
        return Err(Failure::refused(problem));
    }
    create_backup(&data, destination)?;
    let path = destination.display().to_string();
    output.result(
        &BackupOutput {
            schema: BACKUP_OUTPUT_SCHEMA,
            to: &path,
        },
        &format!("backup written to {path}"),
    )?;
    Ok(process::ExitCode::SUCCESS)
}

/// Restores a fully validated backup into a target with neither a database
/// nor an artifact tree.
pub(in crate::cli) fn run_restore(
    output: super::super::output::Output,
    source: &Path,
) -> Result<process::ExitCode, Failure> {
    let environment = Environment::current();
    let data = paths::data_dir(&environment).map_err(|error| Failure::failed_by(&error))?;
    let manifest = validate_backup(source)?;
    restore_files(source, &data, &manifest)?;
    let path = source.display().to_string();
    output.result(
        &RestoreOutput {
            schema: RESTORE_OUTPUT_SCHEMA,
            from: &path,
            artifacts: manifest.artifacts.len(),
        },
        &format!("kernel restored from {path}"),
    )?;
    Ok(process::ExitCode::SUCCESS)
}

/// Creates a backup from an existing database, keeping its snapshot and
/// artifact records consistent without opening the source for writing.
fn create_backup(data: &Path, destination: &Path) -> Result<(), Failure> {
    let source_path = data.join(DATABASE);
    source_file(&source_path)?;
    create_destination(destination)?;
    create_private_dir(&destination.join(ARTIFACTS))
        .map_err(|error| failed_io(&destination.join(ARTIFACTS), "create", &error))?;

    let source = Connection::open_with_flags(&source_path, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| Failure::failed_by(&error))?;
    let destination_database = destination.join(DATABASE);
    create_private_file(&destination_database)
        .map_err(|error| failed_io(&destination_database, "create", &error))?;
    let mut copy =
        Connection::open_with_flags(&destination_database, OpenFlags::SQLITE_OPEN_READ_WRITE)
            .map_err(|error| Failure::failed_by(&error))?;
    {
        let backup = Backup::new(&source, &mut copy).map_err(|error| Failure::failed_by(&error))?;
        backup
            .run_to_completion(128, Duration::from_millis(20), None)
            .map_err(|error| Failure::failed_by(&error))?;
    }
    let migrations = recorded_migrations(&copy).map_err(|error| Failure::failed_by(&error))?;
    copy.pragma_update(None, "journal_mode", "DELETE")
        .map_err(|error| Failure::failed_by(&error))?;
    copy.close()
        .map_err(|(_, error)| Failure::failed_by(&error))?;
    drop(source);

    let artifacts = copy_artifacts(&destination_database, data, destination)?;
    let (database_digest, database_size) = hash_file(&destination_database)
        .map_err(|error| failed_io(&destination_database, "read", &error))?;
    let manifest = Manifest {
        schema: BACKUP_SCHEMA.to_owned(),
        created_at: timestamp(),
        maestro_version: env!("CARGO_PKG_VERSION").to_owned(),
        migrations,
        database: FileRecord {
            path: DATABASE.to_owned(),
            sha256: database_digest,
            size: database_size,
        },
        artifacts,
    };
    let bytes = serde_json::to_vec_pretty(&manifest).map_err(|error| Failure::failed_by(&error))?;
    write_private(&destination.join(MANIFEST), &bytes)
        .map_err(|error| failed_io(&destination.join(MANIFEST), "write", &error))
}

/// Copies the artifacts recorded by the database snapshot, never opening the
/// source database for writing.
fn copy_artifacts(
    snapshot: &Path,
    data: &Path,
    destination: &Path,
) -> Result<Vec<FileRecord>, Failure> {
    let connection = Connection::open_with_flags(snapshot, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|error| Failure::failed_by(&error))?;
    let mut statement = connection
        .prepare("SELECT digest, bytes FROM artifacts ORDER BY digest")
        .map_err(|error| Failure::failed_by(&error))?;
    let entries: Vec<(String, i64)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|error| Failure::failed_by(&error))?
        .collect::<rusqlite::Result<_>>()
        .map_err(|error| Failure::failed_by(&error))?;
    drop(statement);

    let mut artifacts = Vec::with_capacity(entries.len());
    for (text, recorded_size) in entries {
        let digest = Digest::parse(&text).map_err(|error| Failure::failed_by(&error))?;
        let manifest_path = artifact_manifest_path(&digest);
        let relative = native_path(&manifest_path);
        let source_path = data.join(&relative);
        let destination_path = destination.join(&relative);
        source_file(&source_path)?;
        create_private_dir_all(destination_path.parent().unwrap_or(destination))
            .map_err(|error| failed_io(&destination_path, "create its directory", &error))?;
        let (found, size) = copy_hash(&source_path, &destination_path)
            .map_err(|error| failed_io(&source_path, "copy", &error))?;
        if found != text || i64::try_from(size).ok() != Some(recorded_size) {
            return Err(Failure::failed(format!(
                "artifact sha256:{text} does not match its database record"
            )));
        }
        artifacts.push(FileRecord {
            path: manifest_path,
            sha256: found,
            size,
        });
    }
    Ok(artifacts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn copy_artifacts_rejects_a_digest_mismatch_with_matching_size() {
        let root = super::super::test_support::Scratch::new("copy-artifact");
        let data = root.path().join("data");
        let snapshot = root.path().join(DATABASE);
        let destination = root.path().join("backup");
        fs::create_dir_all(&data).unwrap();

        let digest = Digest::of(b"right");
        let relative = native_path(&artifact_manifest_path(&digest));
        let source = data.join(&relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, b"wrong").unwrap();

        let connection = Connection::open(&snapshot).unwrap();
        connection
            .execute("CREATE TABLE artifacts (digest TEXT, bytes INTEGER)", [])
            .unwrap();
        connection
            .execute(
                "INSERT INTO artifacts (digest, bytes) VALUES (?1, ?2)",
                (digest.as_str(), 5_i64),
            )
            .unwrap();
        drop(connection);

        let result = copy_artifacts(&snapshot, &data, &destination);
        assert!(matches!(result, Err(Failure::Failed(_))));
    }
}
