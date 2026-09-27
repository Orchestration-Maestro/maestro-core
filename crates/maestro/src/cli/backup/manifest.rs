//! Backup manifest parsing, path safety and integrity checks.

use super::{
    filesystem::{has_multiple_links, hash_file, refused_io},
    names::{ARTIFACTS, DATABASE, MANIFEST},
};
use crate::failure::Failure;
use maestro_kernel::{artifact::Digest, store};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs, iter,
    path::{Path, PathBuf},
};

/// Schema version recorded by a kernel backup manifest.
pub(super) const BACKUP_SCHEMA: &str = "maestro-backup/1";

/// The versioned manifest for a kernel backup.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    /// The backup schema version.
    pub(super) schema: String,
    /// Creation time in UTC.
    pub(super) created_at: String,
    /// Version of the `maestro` binary that made the backup.
    pub(super) maestro_version: String,
    /// Migrations recorded by the database.
    pub(super) migrations: Vec<String>,
    /// Digest and size of the database file.
    pub(super) database: FileRecord,
    /// Digest and size of each artifact file.
    pub(super) artifacts: Vec<FileRecord>,
}

/// A path and integrity record for one backup file.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct FileRecord {
    /// Relative path within the backup.
    pub(super) path: String,
    /// Lowercase SHA-256 digest.
    pub(super) sha256: String,
    /// File size in bytes.
    pub(super) size: u64,
}

/// Checks every path, file, migration and artifact record in a backup.
pub(super) fn validate_backup(source: &Path) -> Result<Manifest, Failure> {
    let root_metadata =
        fs::symlink_metadata(source).map_err(|error| refused_io(source, "inspect", &error))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(Failure::refused(format!(
            "backup {} is not a directory",
            source.display()
        )));
    }
    require_directory(&source.join(ARTIFACTS))?;
    let manifest_path = source.join(MANIFEST);
    require_file(&manifest_path)?;
    let bytes =
        fs::read(&manifest_path).map_err(|error| refused_io(&manifest_path, "read", &error))?;
    let manifest: Manifest =
        serde_json::from_slice(&bytes).map_err(|error| Failure::refused_by(&error))?;
    if manifest.schema != BACKUP_SCHEMA
        || manifest.created_at.is_empty()
        || manifest.maestro_version.is_empty()
    {
        return Err(Failure::refused(
            "backup manifest has an unsupported or incomplete header",
        ));
    }

    let database_path = safe_relative_path(&manifest.database.path)?;
    if database_path != Path::new(DATABASE) {
        return Err(Failure::refused(
            "backup database path must be kernel.sqlite3",
        ));
    }
    let mut expected = BTreeSet::from([PathBuf::from(MANIFEST), database_path]);
    for artifact in &manifest.artifacts {
        let relative = safe_relative_path(&artifact.path)?;
        let digest =
            Digest::parse(&artifact.sha256).map_err(|error| Failure::refused_by(&error))?;
        if relative != artifact_relative_path(&digest) {
            return Err(Failure::refused(format!(
                "artifact path {} does not match its digest",
                artifact.path
            )));
        }
        if !expected.insert(relative) {
            return Err(Failure::refused(format!(
                "backup manifest lists {} more than once",
                artifact.path
            )));
        }
    }
    let actual = backup_files(source)?;
    if actual != expected {
        let missing = expected.difference(&actual).next();
        let extra = actual.difference(&expected).next();
        let detail = match (missing, extra) {
            (Some(path), _) => format!("backup is missing {}", path.display()),
            (_, Some(path)) => format!("backup contains unlisted file {}", path.display()),
            (None, None) => "backup files do not match its manifest".to_owned(),
        };
        return Err(Failure::refused(detail));
    }

    for record in iter::once(&manifest.database).chain(&manifest.artifacts) {
        verify_file(&source.join(&record.path), &record.sha256, record.size)?;
    }
    let connection =
        Connection::open_with_flags(source.join(DATABASE), OpenFlags::SQLITE_OPEN_READ_ONLY)
            .map_err(|error| Failure::refused_by(&error))?;
    if recorded_migrations(&connection).map_err(|error| Failure::refused_by(&error))?
        != manifest.migrations
    {
        return Err(Failure::refused(
            "backup migration list does not match its database",
        ));
    }
    store::pending_migrations(source).map_err(|error| Failure::refused_by(&error))?;
    verify_artifact_records(&connection, &manifest.artifacts)?;
    Ok(manifest)
}

/// Lists all regular files below a backup and rejects links or special files.
fn backup_files(root: &Path) -> Result<BTreeSet<PathBuf>, Failure> {
    fn visit(root: &Path, directory: &Path, found: &mut BTreeSet<PathBuf>) -> Result<(), Failure> {
        for entry in
            fs::read_dir(directory).map_err(|error| refused_io(directory, "list", &error))?
        {
            let entry = entry.map_err(|error| refused_io(directory, "list", &error))?;
            let path = entry.path();
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| refused_io(&path, "inspect", &error))?;
            if metadata.file_type().is_symlink() {
                return Err(Failure::refused(format!(
                    "backup contains symbolic link {}",
                    path.display()
                )));
            }
            if metadata.is_dir() {
                visit(root, &path, found)?;
                continue;
            }
            if !metadata.is_file() {
                return Err(Failure::refused(format!(
                    "backup contains non-file {}",
                    path.display()
                )));
            }
            if has_multiple_links(&path, &metadata)
                .map_err(|error| refused_io(&path, "inspect", &error))?
            {
                return Err(Failure::refused(format!(
                    "backup contains hard link {}",
                    path.display()
                )));
            }
            let relative = path
                .strip_prefix(root)
                .map_err(|error| Failure::refused_by(&error))?;
            found.insert(relative.to_path_buf());
        }
        Ok(())
    }
    let mut found = BTreeSet::new();
    visit(root, root, &mut found)?;
    Ok(found)
}

/// Parses a relative path that cannot leave the backup directory.
fn safe_relative_path(text: &str) -> Result<PathBuf, Failure> {
    let bytes = text.as_bytes();
    let windows_drive_prefix =
        bytes.first().is_some_and(u8::is_ascii_alphabetic) && bytes.get(1) == Some(&b':');
    if text.contains('\\')
        || windows_drive_prefix
        || text
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return Err(Failure::refused(format!("unsafe backup path {text:?}")));
    }
    Ok(Path::new(text).to_path_buf())
}

/// Checks that `path` is a standalone file with the recorded digest and size.
fn verify_file(path: &Path, expected_digest: &str, expected_size: u64) -> Result<(), Failure> {
    require_file(path)?;
    let (digest, size) = hash_file(path).map_err(|error| refused_io(path, "read", &error))?;
    if size != expected_size {
        return Err(Failure::refused(format!(
            "backup file {} has the wrong size",
            path.display()
        )));
    }
    if digest != expected_digest {
        return Err(Failure::refused(format!(
            "backup file {} has the wrong digest",
            path.display()
        )));
    }
    Ok(())
}

/// Checks the manifest artifact set against the database records.
fn verify_artifact_records(connection: &Connection, records: &[FileRecord]) -> Result<(), Failure> {
    let mut statement = connection
        .prepare("SELECT digest, bytes FROM artifacts ORDER BY digest")
        .map_err(|error| Failure::refused_by(&error))?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|error| Failure::refused_by(&error))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|error| Failure::refused_by(&error))?;
    let database: BTreeSet<(String, u64)> = rows
        .into_iter()
        .map(|(digest, size)| {
            u64::try_from(size)
                .map(|size| (digest, size))
                .map_err(|_| Failure::refused("invalid artifact size"))
        })
        .collect::<Result<_, _>>()?;
    let listed: BTreeSet<(String, u64)> = records
        .iter()
        .map(|record| (record.sha256.clone(), record.size))
        .collect();
    if database != listed {
        return Err(Failure::refused(
            "backup artifact list does not match its database",
        ));
    }
    Ok(())
}

/// Requires `path` to be a real directory, not a symbolic link.
fn require_directory(path: &Path) -> Result<(), Failure> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| refused_io(path, "inspect", &error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(Failure::refused(format!(
            "backup path {} is not a directory",
            path.display()
        )));
    }
    Ok(())
}

/// Requires `path` to be a standalone regular file.
fn require_file(path: &Path) -> Result<(), Failure> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| refused_io(path, "inspect", &error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Failure::refused(format!(
            "backup path {} is not a regular file",
            path.display()
        )));
    }
    if has_multiple_links(path, &metadata).map_err(|error| refused_io(path, "inspect", &error))? {
        return Err(Failure::refused(format!(
            "backup contains hard link {}",
            path.display()
        )));
    }
    Ok(())
}

/// Returns the artifact store path for `digest`.
pub(super) fn artifact_relative_path(digest: &Digest) -> PathBuf {
    let mut characters = digest.as_str().chars();
    let first: String = characters.by_ref().take(2).collect();
    let second: String = characters.by_ref().take(2).collect();
    Path::new(ARTIFACTS)
        .join("sha256")
        .join(first)
        .join(second)
        .join(digest.as_str())
}

/// Reads the migration names recorded by a database in name order.
pub(super) fn recorded_migrations(connection: &Connection) -> rusqlite::Result<Vec<String>> {
    connection
        .prepare("SELECT name FROM migrations ORDER BY name")?
        .query_map([], |row| row.get(0))?
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::test_support::Scratch;
    use super::*;

    #[test]
    fn safe_relative_path_rejects_nonportable_or_non_normal_components() {
        let invalid = [
            "",
            r"artifacts\sha256\digest",
            "/absolute/path",
            "C:drive-relative",
            "C:/drive-absolute",
            "artifacts//digest",
            "artifacts/./digest",
            "artifacts/../outside",
            "artifacts/",
        ];
        for path in invalid {
            assert!(safe_relative_path(path).is_err(), "{path:?}");
        }
        assert_eq!(
            safe_relative_path("artifacts/sha256/ab/cd/digest").unwrap(),
            Path::new("artifacts/sha256/ab/cd/digest")
        );
    }

    #[test]
    fn file_and_directory_requirements_reject_wrong_types_and_links() {
        let root = Scratch::new("types");
        let directory = root.path().join("directory");
        fs::create_dir(&directory).unwrap();
        assert!(require_directory(&directory).is_ok());
        assert!(require_file(&directory).is_err());

        let file = root.path().join("file");
        fs::write(&file, b"regular file").unwrap();
        assert!(require_file(&file).is_ok());
        assert!(require_directory(&file).is_err());

        #[cfg(unix)]
        {
            use std::os::unix::{fs::symlink, net::UnixListener};

            let directory_link = root.path().join("directory-link");
            symlink(&directory, &directory_link).unwrap();
            assert!(require_directory(&directory_link).is_err());

            let file_link = root.path().join("file-link");
            symlink(&file, &file_link).unwrap();
            assert!(require_file(&file_link).is_err());

            let hard_link = root.path().join("hard-link");
            fs::hard_link(&file, &hard_link).unwrap();
            assert!(require_file(&file).is_err());

            let socket = root.path().join("socket");
            let listener = UnixListener::bind(&socket).unwrap();
            assert!(require_file(&socket).is_err());
            drop(listener);
        }
    }

    #[test]
    fn verify_file_rejects_same_size_digest_mismatch() {
        let root = Scratch::new("digest");
        let file = root.path().join("artifact");
        fs::write(&file, b"wrong").unwrap();
        let expected = Digest::of(b"right");
        assert!(verify_file(&file, expected.as_str(), 5).is_err());
    }
}
