//! Immutable snapshots, accessed through directory handles without following symlinks.
use crate::document::CanonicalDocument;
use crate::error::Error;
use crate::hashing::digest;
use crate::replay::validate_document;
use rustix::fd::OwnedFd;
use rustix::fs::{AtFlags, Mode, OFlags, linkat, mkdirat, openat, unlinkat};
use rustix::io::Errno;
use std::{
    ffi::OsStr,
    fs::File,
    io::{self, ErrorKind, Read, Write},
    path::{Component, Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A per-process counter that makes each pending file name unique.
static TEMP_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Save exact Markdown and JSON in a versioned local directory on Unix.
/// JSON references `original.md` relative to itself. Equal artifacts are reused;
/// existing unequal files, traversal and symlinks are refused, never overwritten.
/// JSON is published last; files and directory entries are synchronized.
///
/// # Errors
/// Returns an error for reference/replay mismatches, unsafe paths or I/O errors.
pub fn save_document(
    document: &CanonicalDocument,
    markdown: &str,
    output: &Path,
) -> Result<PathBuf, Error> {
    verify_replay(document, markdown)?;
    let mut saved = document.clone();
    saved.original_markdown_reference.path = "original.md".into();
    let json = encode(&saved)?;
    let directory = output.join(artifact_suffix(&saved, &json));
    let dir = open_directory(&directory, true).map_err(|error| storage_error(&error))?;
    write_immutable(&dir, "original.md", markdown.as_bytes())
        .map_err(|error| storage_error(&error))?;
    write_immutable(&dir, "canonical.json", &json).map_err(|error| storage_error(&error))?;
    Ok(directory.join("canonical.json"))
}

/// Load a saved snapshot, verifying path identities, exact bytes and deterministic replay.
/// Only the fixed local `original.md` reference is read, never a supplied arbitrary path.
/// Failed-but-consistent documents remain inspectable; callers must check validation status.
/// Authenticity still requires a trusted artifact reference and filesystem permissions.
///
/// # Errors
/// Returns an error for incomplete, unsafe, corrupted or unsupported snapshots.
pub fn load_document(path: &Path) -> Result<(CanonicalDocument, String), Error> {
    if path.file_name().is_none_or(|name| name != "canonical.json") {
        return Err(Error(
            "expected an immutable canonical.json snapshot".into(),
        ));
    }
    let parent = path
        .parent()
        .ok_or_else(|| Error("missing snapshot directory".into()))?;
    let dir = open_directory(parent, false).map_err(|error| storage_error(&error))?;
    let json = read_regular(&dir, "canonical.json").map_err(|error| storage_error(&error))?;
    // The strict mapping visitor also rejects duplicates inside opaque JSON policies.
    serde_json::from_slice::<serde_yaml_ng::Value>(&json)
        .map_err(|_| Error("invalid or duplicate snapshot JSON fields".into()))?;
    let document: CanonicalDocument = serde_json::from_slice(&json)
        .map_err(|_| Error("unsupported or incomplete canonical snapshot".into()))?;
    if document.original_markdown_reference.path != "original.md"
        || !parent.ends_with(artifact_suffix(&document, &json))
        || encode(&document)? != json
    {
        return Err(Error(
            "snapshot reference, path identity or serialization mismatch".into(),
        ));
    }
    let markdown = String::from_utf8(
        read_regular(&dir, "original.md").map_err(|error| storage_error(&error))?,
    )
    .map_err(|_| Error("snapshot Markdown is not valid UTF-8".into()))?;
    verify_replay(&document, &markdown)?;
    Ok((document, markdown))
}

/// The snapshot's JSON: pretty-printed with a final newline, the exact bytes saved and compared.
fn encode(document: &CanonicalDocument) -> Result<Vec<u8>, Error> {
    let mut json = serde_json::to_vec_pretty(document).map_err(|error| Error(error.to_string()))?;
    json.push(b'\n');
    Ok(json)
}

/// The snapshot's directory: the digests of the document, of the revision and of the JSON bytes.
fn artifact_suffix(document: &CanonicalDocument, json: &[u8]) -> PathBuf {
    PathBuf::from(digest(document.document_id.as_bytes()))
        .join(digest(document.revision_id.as_bytes()))
        .join(digest(json))
}

/// Refuse a document that deterministic replay of its Markdown does not reproduce.
fn verify_replay(document: &CanonicalDocument, markdown: &str) -> Result<(), Error> {
    if validate_document(document, markdown)
        .iter()
        .any(|finding| matches!(finding.code.as_str(), "canonical_mismatch" | "replay_error"))
    {
        return Err(Error(
            "snapshot differs from deterministic source replay".into(),
        ));
    }
    Ok(())
}

/// The refusal for a snapshot file that cannot be read or written.
fn storage_error(error: &io::Error) -> Error {
    Error(format!("cannot access immutable snapshot: {error}"))
}

// Directory-relative syscalls close the check-then-open ancestor/symlink race.
// The local Unix filesystem must support hard links and directory fsync.
/// Open a directory one component at a time without following links, creating missing components
/// when asked; parent traversal is refused.
fn open_directory(path: &Path, create: bool) -> io::Result<File> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
    {
        return Err(io::Error::other(
            "snapshot path contains parent traversal or a prefix",
        ));
    }
    let mut directory = File::open(if path.is_absolute() { "/" } else { "." })?;
    for component in path.components() {
        if let Component::Normal(name) = component {
            directory = File::from(open_child(&directory, name, create)?);
        }
    }
    Ok(directory)
}

/// Open one child directory without following a link, creating it first when asked and absent.
fn open_child(directory: &File, name: &OsStr, create: bool) -> io::Result<OwnedFd> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    match openat(directory, name, flags, Mode::empty()) {
        Ok(child) => Ok(child),
        Err(Errno::NOENT) if create => {
            match mkdirat(directory, name, Mode::RWXU) {
                Ok(()) => directory.sync_all()?,
                Err(Errno::EXIST) => {}
                Err(error) => return Err(error.into()),
            }
            Ok(openat(directory, name, flags, Mode::empty())?)
        }
        Err(error) => Err(error.into()),
    }
}

/// The bytes of a regular file in the directory, never read through a link.
fn read_regular(directory: &File, name: &str) -> io::Result<Vec<u8>> {
    let fd = openat(
        directory,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let mut file = File::from(fd);
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("artifact is not a regular file"));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Whether the file already holds exactly these bytes; different bytes are an error, never
/// overwritten.
fn verify_existing(directory: &File, name: &str, bytes: &[u8]) -> io::Result<bool> {
    match read_regular(directory, name) {
        Ok(existing) if existing == bytes => Ok(true),
        Ok(_) => Err(io::Error::other(
            "existing immutable artifact differs; refusing overwrite",
        )),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Write a file once: to a pending name, synced, then linked into place; an existing equal file is
/// kept and a different one refused.
fn write_immutable(directory: &File, name: &str, bytes: &[u8]) -> io::Result<()> {
    if verify_existing(directory, name, bytes)? {
        return Ok(());
    }
    let temporary = format!(
        ".{name}.pending-{}-{}",
        process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let fd = openat(
        directory,
        temporary.as_str(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::RUSR | Mode::WUSR,
    )?;
    let mut file = File::from(fd);
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        match linkat(
            directory,
            temporary.as_str(),
            directory,
            name,
            AtFlags::empty(),
        ) {
            Ok(()) => directory.sync_all(),
            Err(Errno::EXIST) if verify_existing(directory, name, bytes)? => Ok(()),
            Err(error) => Err(error.into()),
        }
    })();
    let cleanup =
        unlinkat(directory, temporary.as_str(), AtFlags::empty()).map_err(io::Error::from);
    result.and(cleanup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CanonicalizeInput;
    use crate::pipeline::canonicalize;
    use rustix::fs::mkfifoat;
    use std::{
        env, fs,
        os::unix::fs::symlink,
        sync::{Barrier, mpsc},
        thread,
        time::Duration,
    };

    /// A new empty directory; its name does not draw on `TEMP_SEQUENCE`, which pending names use.
    fn scratch() -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let root = env::temp_dir().join(format!(
            "canonical-store-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        root
    }

    /// `read_regular` on its own thread: the test fails, rather than hangs, if the open blocks.
    fn read_without_blocking(directory: &File, name: &'static str) -> io::Result<Vec<u8>> {
        let directory = directory.try_clone().unwrap();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || sender.send(read_regular(&directory, name)));
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("opening the artifact blocked")
    }

    #[test]
    fn directory_handles_resist_ancestor_replacement_and_reject_special_files() {
        let root = scratch();
        let output = root.join("output");
        let directory = open_directory(&output, true).unwrap();
        fs::rename(&output, root.join("moved")).unwrap();
        fs::create_dir(root.join("outside")).unwrap();
        fs::write(root.join("outside/original.md"), "unchanged").unwrap();
        symlink(root.join("outside"), &output).unwrap();
        write_immutable(&directory, "original.md", b"safe").unwrap();
        assert_eq!(read_regular(&directory, "original.md").unwrap(), b"safe");
        assert_eq!(fs::read(root.join("moved/original.md")).unwrap(), b"safe");
        assert_eq!(
            fs::read(root.join("outside/original.md")).unwrap(),
            b"unchanged"
        );
        assert!(open_directory(&output, false).is_err());
        mkfifoat(&directory, "fifo", Mode::RUSR | Mode::WUSR).unwrap();
        assert!(read_without_blocking(&directory, "fifo").is_err());
        assert!(read_regular(&directory, ".").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_snapshot_copied_out_of_its_identity_directory_is_refused() {
        let root = scratch();
        let markdown = "Moved\n";
        let document = canonicalize(CanonicalizeInput::new(markdown, "moved.md")).unwrap();
        let saved = save_document(&document, markdown, &root.join("out")).unwrap();
        load_document(&saved).unwrap();
        let copy = root.join("copy");
        fs::create_dir(&copy).unwrap();
        for name in ["canonical.json", "original.md"] {
            fs::copy(saved.with_file_name(name), copy.join(name)).unwrap();
        }
        let error = load_document(&copy.join("canonical.json")).unwrap_err();
        assert!(error.to_string().contains("path identity"), "{error}");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn only_directories_open_and_only_saving_creates_them() {
        let root = scratch();
        fs::write(root.join("plain"), "not a directory").unwrap();
        assert!(open_directory(&root.join("plain"), false).is_err());
        assert!(open_directory(&root.join("absent/deeper"), false).is_err());
        assert!(!root.join("absent").exists());
        open_directory(&root.join("absent/deeper"), true).unwrap();
        assert!(root.join("absent/deeper").is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn artifacts_are_never_read_through_a_link() {
        let root = scratch();
        fs::write(root.join("target.md"), "linked").unwrap();
        symlink(root.join("target.md"), root.join("original.md")).unwrap();
        let directory = open_directory(&root, false).unwrap();
        assert!(read_regular(&directory, "original.md").is_err());
        assert_eq!(read_regular(&directory, "target.md").unwrap(), b"linked");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_directory_in_place_of_an_artifact_is_named_and_kept() {
        let root = scratch();
        fs::create_dir(root.join("original.md")).unwrap();
        let directory = open_directory(&root, false).unwrap();
        let error = write_immutable(&directory, "original.md", b"text").unwrap_err();
        assert_eq!(error.to_string(), "artifact is not a regular file");
        assert!(root.join("original.md").is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_planted_pending_name_is_never_followed() {
        let root = scratch();
        fs::write(root.join("victim"), "unchanged").unwrap();
        // Other tests may take sequence numbers meanwhile; plant well past the next one.
        let next = TEMP_SEQUENCE.load(Ordering::Relaxed);
        for sequence in next..next + 64 {
            let pending = format!(".original.md.pending-{}-{sequence}", process::id());
            symlink(root.join("victim"), root.join(pending)).unwrap();
        }
        let directory = open_directory(&root, false).unwrap();
        assert!(write_immutable(&directory, "original.md", b"planted").is_err());
        assert_eq!(fs::read(root.join("victim")).unwrap(), b"unchanged");
        assert!(fs::symlink_metadata(root.join("original.md")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn of_two_racing_writers_of_different_bytes_one_is_refused() {
        let root = scratch();
        // Both writers usually pass the first check before either links: the loser then meets
        // the winner's file at the link and must compare it, not accept it.
        for round in 0..50 {
            let directory = open_directory(&root.join(round.to_string()), true).unwrap();
            let accepted = race_writers(&directory);
            assert_eq!(
                accepted.iter().filter(|&&won| won).count(),
                1,
                "round {round}"
            );
            let winner: &[u8] = if accepted[0] { b"left" } else { b"right" };
            let artifact = root.join(round.to_string()).join("artifact");
            assert_eq!(fs::read(artifact).unwrap(), winner);
        }
        fs::remove_dir_all(root).unwrap();
    }

    /// Two threads released together, each writing different bytes to `artifact`: which of the
    /// left and right writers were accepted.
    fn race_writers(directory: &File) -> Vec<bool> {
        let barrier = Barrier::new(2);
        let write = |bytes: &[u8]| {
            barrier.wait();
            write_immutable(directory, "artifact", bytes).is_ok()
        };
        thread::scope(|scope| {
            let left = scope.spawn(|| write(b"left"));
            let right = scope.spawn(|| write(b"right"));
            vec![left.join().unwrap(), right.join().unwrap()]
        })
    }
}
