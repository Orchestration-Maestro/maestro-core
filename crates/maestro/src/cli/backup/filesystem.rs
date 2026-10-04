//! Filesystem operations for private backup and restore files.

use super::super::filesystem::has_multiple_links;
use crate::failure::Failure;
use sha2::{Digest as _, Sha256};
use std::{
    fs,
    fs::{DirBuilder, File, OpenOptions},
    io::{self, Read as _, Write as _},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Reports whether `path` cannot be used as an empty backup destination.
pub(super) fn directory_problem(path: &Path) -> Result<Option<String>, Failure> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => Ok(Some(
            format!("backup destination {} is not a directory", path.display()),
        )),
        Ok(_) => match fs::read_dir(path) {
            Ok(mut entries) => match entries.next() {
                None => Ok(None),
                Some(_) => Ok(Some(format!(
                    "backup destination {} is not empty",
                    path.display()
                ))),
            },
            Err(error) => Err(failed_io(path, "list", &error)),
        },
        Err(error) => {
            let error = normalize_path_error(path, error);
            if error.kind() == io::ErrorKind::NotFound {
                Ok(None)
            } else {
                Err(failed_io(path, "inspect", &error))
            }
        }
    }
}

/// Creates `path` as a private directory when it does not exist.
pub(super) fn create_destination(path: &Path) -> Result<(), Failure> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => return Ok(()),
        Ok(_) => {
            return Err(Failure::refused(format!(
                "backup destination {} is not a directory",
                path.display()
            )));
        }
        Err(error) => {
            let error = normalize_path_error(path, error);
            if error.kind() != io::ErrorKind::NotFound {
                return Err(failed_io(path, "inspect", &error));
            }
        }
    }
    if let Some(parent) = path.parent() {
        create_private_dir_all(parent).map_err(|error| failed_io(parent, "create", &error))?;
    }
    create_private_dir(path).map_err(|error| failed_io(path, "create", &error))
}

/// Creates the directory tree without following symbolic links.
pub(super) fn create_private_dir_all(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(io::Error::other("not a directory")),
        Err(error) => {
            let error = normalize_path_error(path, error);
            if error.kind() != io::ErrorKind::NotFound {
                return Err(error);
            }
            if let Some(parent) = path.parent()
                && parent != path
            {
                create_private_dir_all(parent)?;
            }
            create_private_dir(path)
        }
    }
}

/// Normalizes a missing-path error when a parent blocks no-follow path traversal.
pub(super) fn normalize_path_error(path: &Path, error: io::Error) -> io::Error {
    if error.kind() == io::ErrorKind::NotFound && has_blocking_parent(path) {
        io::Error::new(io::ErrorKind::NotADirectory, error)
    } else {
        error
    }
}

/// Checks whether the nearest existing ancestor is not a directory: a missing
/// path beneath a file is unreachable, not absent. Symlinked directories, such
/// as macOS's `/var`, are followed, so a missing path under them stays missing.
pub(super) fn has_blocking_parent(path: &Path) -> bool {
    let mut parent = path.parent();
    while let Some(candidate) = parent {
        match fs::metadata(candidate) {
            Ok(metadata) => return !metadata.is_dir(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return true,
        }
        parent = candidate.parent();
    }
    false
}

/// Creates a directory with owner-only permissions on Unix.
pub(super) fn create_private_dir(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    let builder = {
        use std::os::unix::fs::DirBuilderExt as _;
        let mut builder = DirBuilder::new();
        builder.mode(0o700);
        builder
    };
    #[cfg(not(unix))]
    let builder = DirBuilder::new();
    builder.create(path)
}

/// Creates a new file with owner-only permissions on Unix.
pub(super) fn create_private_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    options.open(path)
}

/// Writes and syncs `bytes` to a new private file.
pub(super) fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = create_private_file(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

/// Copies `source` and returns the copied file's SHA-256 digest and size.
pub(super) fn copy_hash(source: &Path, destination: &Path) -> io::Result<(String, u64)> {
    let mut input = File::open(source)?;
    let mut output = create_private_file(destination)?;
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let bytes = buffer
            .get(..read)
            .ok_or_else(|| io::Error::other("read exceeded buffer capacity"))?;
        output.write_all(bytes)?;
        hash.update(bytes);
        size = size
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| io::Error::other("file size exceeds u64"))?;
    }
    output.sync_all()?;
    Ok((hex_digest(hash.finalize().as_ref()), size))
}

/// Returns a file's SHA-256 digest and size.
pub(super) fn hash_file(path: &Path) -> io::Result<(String, u64)> {
    let mut input = File::open(path)?;
    let mut hash = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = input.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        let bytes = buffer
            .get(..read)
            .ok_or_else(|| io::Error::other("read exceeded buffer capacity"))?;
        hash.update(bytes);
        size = size
            .checked_add(u64::try_from(read).unwrap_or(u64::MAX))
            .ok_or_else(|| io::Error::other("file size exceeds u64"))?;
    }
    Ok((hex_digest(hash.finalize().as_ref()), size))
}

/// Encodes digest bytes as lowercase hexadecimal.
fn hex_digest(bytes: &[u8]) -> String {
    bytes
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0x0f])
        .filter_map(|nibble| char::from_digit(u32::from(nibble), 16))
        .collect()
}

/// Requires `path` to be a standalone regular file.
pub(super) fn source_file(path: &Path) -> Result<(), Failure> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| failed_io(path, "inspect", &error))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(Failure::failed(format!(
            "kernel file {} is not a standalone regular file",
            path.display()
        )));
    }
    if has_multiple_links(path, &metadata).map_err(|error| failed_io(path, "inspect", &error))? {
        return Err(Failure::failed(format!(
            "kernel file {} is not a standalone regular file",
            path.display()
        )));
    }
    Ok(())
}

/// Returns the current UTC time in RFC 3339 milliseconds.
pub(super) fn timestamp() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    timestamp_for(elapsed)
}

/// Formats elapsed time since the Unix epoch as RFC 3339 milliseconds.
pub(super) fn timestamp_for(elapsed: Duration) -> String {
    let days = i64::try_from(elapsed.as_secs() / 86_400).unwrap_or(i64::MAX);
    let second = elapsed.as_secs() % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_piece = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_piece + 2) / 5 + 1;
    let month = month_piece + if month_piece < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    let hour = second / 3_600;
    let minute = second % 3_600 / 60;
    let seconds = second % 60;
    let millis = elapsed.subsec_millis();
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{seconds:02}.{millis:03}Z")
}

/// Builds an operation failure naming its path and I/O error.
pub(super) fn failed_io(path: &Path, action: &str, error: &io::Error) -> Failure {
    Failure::failed(format!("cannot {action} {}: {error}", path.display()))
}

/// Builds a backup-input refusal naming its path and I/O error.
pub(super) fn refused_io(path: &Path, action: &str, error: &io::Error) -> Failure {
    Failure::refused(format!(
        "cannot {action} backup {}: {error}",
        path.display()
    ))
}
