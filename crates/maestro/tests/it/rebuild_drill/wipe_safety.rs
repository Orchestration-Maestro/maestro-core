//! Ownership checks for the destructive kernel and Qdrant portions.

use maestro_kernel::gateway::Url;
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process,
};

const QDRANT_URL: &str = "http://127.0.0.1:16534";
#[cfg(target_os = "linux")]
const LOCAL_MARKER: &str = ".maestro-t033b-owner";
const CI_OWNER: &str = "github-actions-service/qdrant";

pub(super) struct QdrantOwner {
    pub(super) url: String,
    pub(super) owner: String,
    #[cfg(target_os = "linux")]
    pub(super) storage: Option<PathBuf>,
    #[cfg(target_os = "linux")]
    pub(super) pid: Option<String>,
    pub(super) github_actions: bool,
    pub(super) runner_temp: Option<PathBuf>,
}

/// Proves the disposable endpoint belongs to this invocation or its CI job.
pub(super) fn require_qdrant_owner() -> Result<QdrantOwner, String> {
    let owner = QdrantOwner {
        url: required("MAESTRO_T033B_QDRANT_URL")?,
        owner: required("MAESTRO_T033B_QDRANT_OWNER")?,
        #[cfg(target_os = "linux")]
        storage: env::var_os("MAESTRO_T033B_QDRANT_STORAGE").map(PathBuf::from),
        #[cfg(target_os = "linux")]
        pid: env::var("MAESTRO_T033B_QDRANT_PID").ok(),
        github_actions: env::var("GITHUB_ACTIONS").as_deref() == Ok("true"),
        runner_temp: env::var_os("RUNNER_TEMP").map(PathBuf::from),
    };
    validate_qdrant_owner(&owner)?;
    Ok(owner)
}

fn required(name: &str) -> Result<String, String> {
    env::var(name).map_err(|_| format!("{name} is required for the owned rebuild drill"))
}

fn validate_qdrant_owner(owner: &QdrantOwner) -> Result<(), String> {
    let url =
        Url::parse(&owner.url).map_err(|error| format!("invalid scratch Qdrant URL: {error}"))?;
    if url.scheme() != "http"
        || url.host_str() != Some("127.0.0.1")
        || url.port() != Some(16534)
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(format!(
            "the drill accepts only its scratch endpoint {QDRANT_URL}"
        ));
    }
    if owner.github_actions {
        if owner.owner != CI_OWNER {
            return Err("the GitHub Actions Qdrant service owner marker does not match".to_owned());
        }
        if owner.runner_temp.is_none() {
            return Err("RUNNER_TEMP is required for the disposable CI service".to_owned());
        }
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        let storage = owner
            .storage
            .as_deref()
            .ok_or_else(|| "MAESTRO_T033B_QDRANT_STORAGE is required".to_owned())?;
        let pid = owner
            .pid
            .as_deref()
            .ok_or_else(|| "MAESTRO_T033B_QDRANT_PID is required".to_owned())?;
        validate_local_process(storage, pid, &owner.owner)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = owner;
        Err("local scratch Qdrant ownership is verified on Linux only".to_owned())
    }
}

#[cfg(target_os = "linux")]
fn validate_local_process(storage: &Path, pid: &str, owner: &str) -> Result<(), String> {
    let metadata = fs::symlink_metadata(storage)
        .map_err(|error| format!("cannot inspect scratch Qdrant storage: {error}"))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err("scratch Qdrant storage must be a nonsymlink directory".to_owned());
    }
    let storage = storage
        .canonicalize()
        .map_err(|error| format!("cannot resolve scratch Qdrant storage: {error}"))?;
    let name = storage
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if storage.parent() != Some(Path::new("/tmp")) || !name.starts_with("maestro-t033b-qdrant-") {
        return Err("scratch Qdrant storage must be below /tmp/maestro-t033b-qdrant-*".to_owned());
    }
    let marker = fs::read_to_string(storage.join(LOCAL_MARKER))
        .map_err(|error| format!("scratch Qdrant owner marker is missing: {error}"))?;
    if marker.trim() != owner {
        return Err("scratch Qdrant owner marker does not match this run".to_owned());
    }
    let pid = pid
        .parse::<u32>()
        .map_err(|error| format!("invalid Qdrant PID: {error}"))?;
    let process = PathBuf::from(format!("/proc/{pid}"));
    let command = fs::read(process.join("cmdline"))
        .map_err(|error| format!("cannot inspect Qdrant process {pid}: {error}"))?;
    let environment = fs::read(process.join("environ"))
        .map_err(|error| format!("cannot inspect Qdrant process environment {pid}: {error}"))?;
    let command = String::from_utf8_lossy(&command);
    let environment = String::from_utf8_lossy(&environment);
    if !command.to_ascii_lowercase().contains("qdrant")
        || !environment.contains(&format!(
            "QDRANT__STORAGE__STORAGE_PATH={}",
            storage.display()
        ))
        || !environment.contains("QDRANT__SERVICE__HTTP_PORT=16533")
        || !environment.contains("QDRANT__SERVICE__GRPC_PORT=16534")
    {
        return Err(
            "the recorded process does not own the configured scratch Qdrant service".to_owned(),
        );
    }
    Ok(())
}

/// Checks the disposable kernel and backup layout before any wipe.
pub(super) fn validate_wipe_layout(root: &Path, data: &Path, backup: &Path) -> Result<(), String> {
    for (label, path) in [
        ("scratch root", root),
        ("kernel data", data),
        ("backup", backup),
    ] {
        let metadata = fs::symlink_metadata(path)
            .map_err(|error| format!("cannot inspect {label}: {error}"))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(format!("{label} must be a nonsymlink directory"));
        }
    }
    let root = root
        .canonicalize()
        .map_err(|error| format!("cannot resolve scratch root: {error}"))?;
    let data = data
        .canonicalize()
        .map_err(|error| format!("cannot resolve kernel data: {error}"))?;
    let backup = backup
        .canonicalize()
        .map_err(|error| format!("cannot resolve backup: {error}"))?;
    if !data.starts_with(&root) || !backup.starts_with(&root) || backup.starts_with(&data) {
        return Err(
            "kernel data and backup must be owned siblings beneath the scratch root".to_owned(),
        );
    }
    if data == root || backup == root || data.starts_with(&backup) {
        return Err("kernel data and backup must be separate scratch directories".to_owned());
    }
    Ok(())
}

/// Removes only the database files and artifact tree after checking ownership.
pub(super) fn wipe_kernel(root: &Path, data: &Path, backup: &Path) -> Result<(), String> {
    validate_wipe_layout(root, data, backup)?;
    for name in ["kernel.sqlite3", "kernel.sqlite3-wal", "kernel.sqlite3-shm"] {
        let path = data.join(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err(format!(
                    "refusing to remove non-file kernel entry {}",
                    path.display()
                ));
            }
            Ok(_) => fs::remove_file(&path)
                .map_err(|error| format!("cannot remove owned kernel file {name}: {error}"))?,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot inspect kernel file {name}: {error}")),
        }
    }
    let artifacts = data.join("artifacts");
    match fs::symlink_metadata(&artifacts) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err("refusing to remove a non-directory artifact tree".to_owned());
        }
        Ok(_) => fs::remove_dir_all(&artifacts)
            .map_err(|error| format!("cannot remove owned artifact tree: {error}"))?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect artifact tree: {error}")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::atomic::{AtomicUsize, Ordering},
    };

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = env::temp_dir().join(format!(
                "maestro-t033b-safety-{}-{}",
                process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(path.join("data/maestro")).unwrap();
            fs::create_dir_all(path.join("backup")).unwrap();
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn wipe_layout_rejects_backup_inside_kernel_data() {
        let scratch = Scratch::new();
        let data = scratch.0.join("data/maestro");
        let backup = data.join("backup");
        fs::create_dir(&backup).unwrap();
        assert!(validate_wipe_layout(&scratch.0, &data, &backup).is_err());
    }

    #[test]
    fn wipe_layout_rejects_paths_outside_the_owned_root() {
        let scratch = Scratch::new();
        let outside = env::temp_dir().join(format!("maestro-t033b-outside-{}", process::id()));
        fs::create_dir_all(&outside).unwrap();
        let refused = validate_wipe_layout(&scratch.0, &outside, &scratch.0.join("backup"));
        fs::remove_dir_all(&outside).unwrap();
        assert!(refused.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn wipe_layout_rejects_a_symlinked_data_directory() {
        use std::os::unix::fs::symlink;

        let scratch = Scratch::new();
        let outside = env::temp_dir().join(format!("maestro-t033b-symlink-{}", process::id()));
        fs::create_dir_all(&outside).unwrap();
        let data = scratch.0.join("data/maestro");
        fs::remove_dir(&data).unwrap();
        symlink(&outside, &data).unwrap();
        assert!(validate_wipe_layout(&scratch.0, &data, &scratch.0.join("backup")).is_err());
        fs::remove_file(&data).unwrap();
        fs::remove_dir_all(&outside).unwrap();
    }

    #[test]
    fn qdrant_owner_rejects_default_or_unowned_local_endpoints() {
        let owner = QdrantOwner {
            url: "http://127.0.0.1:6334".to_owned(),
            owner: "someone-else".to_owned(),
            #[cfg(target_os = "linux")]
            storage: None,
            #[cfg(target_os = "linux")]
            pid: None,
            github_actions: false,
            runner_temp: None,
        };
        assert!(validate_qdrant_owner(&owner).is_err());
    }
}
