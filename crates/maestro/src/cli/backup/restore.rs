//! Staged restore checks and the database-last install.

use super::{
    super::failure::Failure,
    filesystem::{copy_hash, create_private_dir, create_private_dir_all, failed_io},
    manifest::Manifest,
    names::{ARTIFACTS, DATABASE},
};
use std::{
    fs, io, iter,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicU64, Ordering},
};

/// Numbers staging directories so concurrent restores do not collide.
static NEXT_STAGE: AtomicU64 = AtomicU64::new(0);

/// Stages and installs a validated backup in an empty kernel slot.
pub(super) fn restore_files(
    source: &Path,
    data: &Path,
    manifest: &Manifest,
) -> Result<(), Failure> {
    ensure_empty_kernel_slot(data)?;
    create_private_dir_all(data).map_err(|error| failed_io(data, "create", &error))?;
    let stage = make_stage(data)?;
    let result = stage_restore(source, data, &stage, manifest, || Ok(()));
    drop(fs::remove_dir_all(&stage));
    result
}

/// Copies validated backup files into the staging directory.
fn stage_restore(
    source: &Path,
    data: &Path,
    stage: &Path,
    manifest: &Manifest,
    before_database_commit: impl FnOnce() -> Result<(), Failure>,
) -> Result<(), Failure> {
    let stage_artifacts = stage.join(ARTIFACTS);
    create_private_dir(&stage_artifacts)
        .map_err(|error| failed_io(&stage_artifacts, "create", &error))?;
    for record in iter::once(&manifest.database).chain(&manifest.artifacts) {
        let source_path = source.join(&record.path);
        let stage_path = stage.join(&record.path);
        create_private_dir_all(stage_path.parent().unwrap_or(stage))
            .map_err(|error| failed_io(&stage_path, "create its directory", &error))?;
        let (digest, size) = copy_hash(&source_path, &stage_path)
            .map_err(|error| failed_io(&source_path, "stage", &error))?;
        if digest != record.sha256 || size != record.size {
            return Err(Failure::refused(format!(
                "backup file {} changed during restore",
                record.path
            )));
        }
    }
    commit_staged(data, stage, before_database_commit)
}

/// Installs staged artifacts and makes the database visible last.
pub(super) fn commit_staged(
    data: &Path,
    stage: &Path,
    before_database_commit: impl FnOnce() -> Result<(), Failure>,
) -> Result<(), Failure> {
    let artifacts = stage.join(ARTIFACTS);
    let target_artifacts = data.join(ARTIFACTS);
    fs::rename(&artifacts, &target_artifacts)
        .map_err(|error| failed_io(&target_artifacts, "install", &error))?;
    if let Err(error) = before_database_commit() {
        drop(fs::remove_dir_all(&target_artifacts));
        return Err(error);
    }
    let staged_database = stage.join(DATABASE);
    let target_database = data.join(DATABASE);
    if let Err(error) = fs::hard_link(&staged_database, &target_database) {
        drop(fs::remove_dir_all(&target_artifacts));
        return Err(failed_io(&target_database, "install", &error));
    }
    Ok(())
}

/// Refuses targets containing any kernel-owned database or artifact entry.
fn ensure_empty_kernel_slot(data: &Path) -> Result<(), Failure> {
    match fs::symlink_metadata(data) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(Failure::refused(format!(
                "data path {} is not a directory",
                data.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(failed_io(data, "inspect", &error)),
    }
    for name in [
        DATABASE,
        "kernel.sqlite3-wal",
        "kernel.sqlite3-shm",
        ARTIFACTS,
    ] {
        let path = data.join(name);
        match fs::symlink_metadata(&path) {
            Ok(_) => {
                return Err(Failure::refused(format!(
                    "restore target already contains {}",
                    path.display()
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(failed_io(&path, "inspect", &error)),
        }
    }
    Ok(())
}

/// Creates a unique private staging directory beside the kernel files.
fn make_stage(data: &Path) -> Result<PathBuf, Failure> {
    for _ in 0..100 {
        let sequence = NEXT_STAGE.fetch_add(1, Ordering::Relaxed);
        let path = data.join(format!(".restore-{}-{sequence}", process::id()));
        match create_private_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(failed_io(&path, "create", &error)),
        }
    }
    Err(Failure::failed(
        "cannot reserve a temporary restore directory",
    ))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::Scratch;
    use super::*;
    use maestro_kernel::artifact::Digest;

    #[test]
    fn empty_slot_requires_a_directory_and_handles_missing_roots() {
        let root = Scratch::new("empty-slot");
        let missing = root.path().join("missing");
        assert!(ensure_empty_kernel_slot(&missing).is_ok());

        let file = root.path().join("file");
        fs::write(&file, b"not a directory").unwrap();
        assert!(matches!(
            ensure_empty_kernel_slot(&file),
            Err(Failure::Refused(_))
        ));

        let blocked = file.join("child");
        assert!(matches!(
            ensure_empty_kernel_slot(&blocked),
            Err(Failure::Failed(_))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn empty_slot_reports_permission_errors_instead_of_treating_them_as_missing() {
        use std::os::unix::fs::PermissionsExt as _;

        let root = Scratch::new("private-slot");
        let data = root.path().join("data");
        fs::create_dir(&data).unwrap();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o0)).unwrap();
        let result = ensure_empty_kernel_slot(&data);
        fs::set_permissions(&data, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(matches!(result, Err(Failure::Failed(_))));
    }

    #[test]
    fn staging_retries_collisions_and_reports_other_errors() {
        let root = Scratch::new("stage");
        let data = root.path().join("data");
        fs::create_dir(&data).unwrap();
        let sequence = NEXT_STAGE.load(Ordering::Relaxed);
        let collision = data.join(format!(".restore-{}-{sequence}", process::id()));
        fs::create_dir(&collision).unwrap();
        let stage = make_stage(&data).unwrap();
        assert_eq!(
            stage,
            data.join(format!(".restore-{}-{}", process::id(), sequence + 1))
        );

        let blocked = root.path().join("not-a-directory");
        fs::write(&blocked, b"file").unwrap();
        let error = make_stage(&blocked).unwrap_err();
        assert!(error.to_string().contains("cannot create"), "{error}");
    }

    #[test]
    fn staging_refuses_a_digest_mismatch_when_the_size_matches() {
        let root = Scratch::new("stage-digest");
        let source = root.path().join("source");
        let data = root.path().join("data");
        let stage = data.join(".restore-test");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&data).unwrap();
        fs::create_dir(&stage).unwrap();

        let database_bytes = b"database";
        fs::write(source.join(DATABASE), database_bytes).unwrap();
        let database_digest = Digest::of(database_bytes);
        let artifact_digest = Digest::of(b"right");
        let artifact_path = super::super::manifest::artifact_relative_path(&artifact_digest);
        fs::create_dir_all(source.join(artifact_path.parent().unwrap())).unwrap();
        fs::write(source.join(&artifact_path), b"wrong").unwrap();
        let manifest = Manifest {
            schema: super::super::manifest::BACKUP_SCHEMA.to_owned(),
            created_at: String::new(),
            maestro_version: String::new(),
            migrations: Vec::new(),
            database: super::super::manifest::FileRecord {
                path: DATABASE.to_owned(),
                sha256: database_digest.as_str().to_owned(),
                size: u64::try_from(database_bytes.len()).unwrap(),
            },
            artifacts: vec![super::super::manifest::FileRecord {
                path: artifact_path.to_string_lossy().into_owned(),
                sha256: artifact_digest.as_str().to_owned(),
                size: 5,
            }],
        };

        let result = stage_restore(&source, &data, &stage, &manifest, || Ok(()));
        assert!(matches!(result, Err(Failure::Refused(_))));
        assert!(!data.join(DATABASE).exists());
        assert!(!data.join(ARTIFACTS).exists());
    }
}
