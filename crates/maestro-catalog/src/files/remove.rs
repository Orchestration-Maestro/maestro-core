//! Remove only committed, digest-matching owned file names.
use super::{
    names::ownership_name,
    plan::{digest, split_path, validate_id, validate_relative_path},
    recovery::state_directory,
};
use maestro_filesystem::Directory;
use serde::Deserialize;
use std::{io, path::Path, str};

/// The durable ownership manifest for one completed operation.
#[derive(Deserialize)]
struct Ownership {
    /// The plan identity the record belongs to.
    id: String,
    /// Root-relative file names and their committed byte digests.
    files: Vec<OwnedFile>,
}

/// A file that the completed plan owns by path and digest.
#[derive(Deserialize)]
struct OwnedFile {
    /// The validated root-relative file name.
    path: String,
    /// SHA-256 of the bytes published by the plan.
    digest: String,
    /// Stable Unix file identity; absent in records from platforms without one.
    #[serde(default)]
    identity: Option<FileIdentity>,
}

/// Stable Unix identity of the opened target.
#[derive(Deserialize)]
struct FileIdentity {
    /// Device number.
    device: u64,
    /// Inode number.
    inode: u64,
}

/// Remove only files whose current bytes match the ownership record for `id`.
///
/// All paths are checked before the first removal, and each name is removed through the shared
/// filesystem's verified quarantine operation. A missing file is treated as prior progress. Created
/// parent directories are intentionally retained. On Windows, std exposes no stable file identity,
/// so a deleted file recreated with identical bytes cannot be distinguished yet; Windows removal
/// remains subject to the recorded file-identity dependency follow-up. On Unix, unlink follows
/// digest verification by name, so an open writer can change bytes after verification.
///
/// # Errors
/// Returns an error for malformed ownership, edited content, unsafe paths, or filesystem failure.
pub fn remove(root: &Path, id: &str) -> io::Result<()> {
    validate_id(id)?;
    let state = state_directory(root)?;
    let name = ownership_name(id);
    let bytes = state.read_regular(&name)?;
    let ownership: Ownership = toml::from_str(str::from_utf8(&bytes).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid ownership record: {error}"),
        )
    })?)
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid ownership record: {error}"),
        )
    })?;
    if ownership.id != id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ownership identity mismatch",
        ));
    }

    let mut verified = Vec::new();
    for owned in &ownership.files {
        validate_relative_path(&owned.path)?;
        let (parent, file_name) = split_path(&owned.path)?;
        let directory = match Directory::open(root, &parent, false) {
            Ok(directory) => directory,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        match directory.read_regular(file_name) {
            Ok(current) if digest(&current) == owned.digest => {
                verified.push((
                    directory,
                    file_name.to_owned(),
                    current,
                    owned
                        .identity
                        .as_ref()
                        .map(|identity| (identity.device, identity.inode)),
                ));
            }
            Ok(_) => {
                return Err(io::Error::other(format!(
                    "owned file changed: {}",
                    owned.path
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    for (directory, file_name, current, identity) in verified {
        directory.remove_verified(&file_name, &current, identity)?;
    }
    state.remove_verified(&name, &bytes, None)?;
    Ok(())
}
