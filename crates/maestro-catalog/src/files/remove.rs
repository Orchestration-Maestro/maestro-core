//! Remove only committed, digest-matching owned file names.
use super::{
    effects,
    names::ownership_name,
    plan::{digest, validate_id, validate_relative_path},
};
use crate::policy::workspace::CheckedTrust;
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
pub fn remove(root: &Path, id: &str, trust: &CheckedTrust<'_>) -> io::Result<()> {
    validate_id(id)?;
    let name = format!(".maestro-files/{}", ownership_name(id));
    effects::check(root, &name, trust)?;
    let bytes = effects::read(root, &name, trust)?;
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
        effects::check(root, &owned.path, trust)?;
        match effects::read(root, &owned.path, trust) {
            Ok(current) if digest(&current) == owned.digest => {
                verified.push((
                    owned.path.clone(),
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
    for (path, current, identity) in verified {
        effects::remove(root, &path, &current, identity, trust)?;
    }
    effects::remove(root, &name, &bytes, None, trust)?;
    Ok(())
}
