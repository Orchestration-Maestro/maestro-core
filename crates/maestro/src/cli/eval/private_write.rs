//! Durable no-overwrite publication for admitted private paths.

use std::{fs, io, path::Path};

/// Writes a new artifact; the callback permits deterministic IO failure tests.
pub(super) fn write(
    path: &Path,
    write_bytes: impl FnOnce(&mut fs::File) -> io::Result<()>,
) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent"))?;
    let temporary = parent.join(format!(".tmp-{}", ulid::Ulid::generate()));
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&temporary)?;
    let written = write_bytes(&mut file).and_then(|()| file.sync_all());
    // Windows requires closing the handle before publishing or deleting it.
    drop(file);
    let placed = written.and_then(|()| fs::hard_link(&temporary, path));
    let cleaned = fs::remove_file(&temporary);
    placed?;
    cleaned?;
    // std cannot sync a directory handle on Windows.
    #[cfg(unix)]
    fs::File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::write;
    use maestro_test_scratch::scratch_directory;
    use std::{
        fs,
        io::{self, Write as _},
    };

    #[test]
    fn graph_eval_failed_private_write_leaves_no_final_and_retry_succeeds() {
        let root = scratch_directory().unwrap();
        let path = root.join("receipt.json");
        let result = write(&path, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("injected before publication"))
        });
        let absent = !path.exists();
        let empty = fs::read_dir(&root).unwrap().next().is_none();
        let retry = write(&path, |file| file.write_all(b"complete"));
        let bytes = fs::read(&path).unwrap();
        fs::remove_dir_all(root).unwrap();
        assert!(result.is_err());
        assert!(absent, "a failed write published the final name");
        assert!(empty, "a failed write leaked its temporary file");
        assert!(retry.is_ok());
        assert_eq!(bytes, b"complete");
    }
}
