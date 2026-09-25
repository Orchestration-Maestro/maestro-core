//! Immutable snapshots, accessed through directory handles without following links.
use crate::document::CanonicalDocument;
use crate::error::Error;
use crate::filesystem::Directory;
use crate::hashing::digest;
use crate::replay::validate_document;
use std::{
    fs::File,
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A per-process counter that makes each pending file name unique.
static TEMP_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Save exact Markdown and JSON in a versioned local directory on Linux, macOS or Windows.
/// JSON references `original.md` relative to itself. Equal artifacts are reused;
/// existing unequal files, traversal and symlinks are refused, never overwritten. `output` is
/// trusted: its links resolve once, and below it no link is followed.
/// JSON is published last; files and directory entries are synchronized, except that Windows
/// cannot flush a directory: there power loss before NTFS commits its journal can drop the
/// newest entry, which the next save recreates (ADR-0018).
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
    let suffix = artifact_suffix(&saved, &json);
    let dir = Directory::open(output, &suffix, true).map_err(|error| storage_error(&error))?;
    write_immutable(&dir, "original.md", markdown.as_bytes())
        .map_err(|error| storage_error(&error))?;
    write_immutable(&dir, "canonical.json", &json).map_err(|error| storage_error(&error))?;
    Ok(output.join(suffix).join("canonical.json"))
}

/// Load a saved snapshot, verifying path identities, exact bytes and deterministic replay.
/// Only the fixed local `original.md` reference is read, never a supplied arbitrary path.
/// The directory three levels above the snapshot is the root the caller saved to: its links
/// resolve once, and the three identity directories below it are never reached through a link.
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
    let root = parent
        .ancestors()
        .nth(3)
        .ok_or_else(|| Error("missing snapshot identity directories".into()))?;
    let below = parent
        .strip_prefix(root)
        .map_err(|error| Error(error.to_string()))?;
    let dir = Directory::open(root, below, false).map_err(|error| storage_error(&error))?;
    let json = dir
        .read_regular("canonical.json")
        .map_err(|error| storage_error(&error))?;
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
        dir.read_regular("original.md")
            .map_err(|error| storage_error(&error))?,
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

/// Whether the file already holds exactly these bytes; different bytes are an error, never
/// overwritten.
fn verify_existing(directory: &Directory, name: &str, bytes: &[u8]) -> io::Result<bool> {
    match directory.read_regular(name) {
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
fn write_immutable(directory: &Directory, name: &str, bytes: &[u8]) -> io::Result<()> {
    if verify_existing(directory, name, bytes)? {
        return Ok(());
    }
    let temporary = format!(
        ".{name}.pending-{}-{}",
        process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let file = directory.create_new(&temporary)?;
    let result = publish(directory, file, &temporary, name, bytes);
    let cleanup = directory.remove_file(&temporary);
    result.and(cleanup)
}

/// Fill and sync the pending file, then link it as `name`: a writer that meets another's file
/// there accepts it only when it holds the same bytes. The pending file is closed on return,
/// before its removal, which Windows refuses for an open file.
fn publish(
    directory: &Directory,
    mut file: File,
    temporary: &str,
    name: &str,
    bytes: &[u8],
) -> io::Result<()> {
    file.write_all(bytes)?;
    file.sync_all()?;
    match directory.link(temporary, name) {
        Ok(()) => Ok(()),
        Err(error)
            if error.kind() == ErrorKind::AlreadyExists
                && verify_existing(directory, name, bytes)? =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::CanonicalizeInput;
    use crate::pipeline::canonicalize;
    #[cfg(windows)]
    use std::path::Component;
    use std::{env, fs, sync::Barrier, thread};
    #[cfg(unix)]
    use std::{
        os::unix::fs::{PermissionsExt, symlink},
        process::Command,
        sync::mpsc,
        time::Duration,
    };

    /// A new empty directory; its name does not draw on `TEMP_SEQUENCE`, which pending names use.
    /// It is the plain temporary path, a link into `/private` on macOS: the root resolves once.
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
    #[cfg(unix)]
    fn read_without_blocking(directory: &Directory, name: &'static str) -> io::Result<Vec<u8>> {
        let directory = directory.try_clone().unwrap();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || sender.send(directory.read_regular(name)));
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("opening the artifact blocked")
    }

    #[cfg(unix)]
    #[test]
    fn directory_handles_resist_ancestor_replacement_and_reject_special_files() {
        let root = scratch();
        let output = root.join("output");
        let directory = Directory::open(&root, Path::new("output"), true).unwrap();
        fs::rename(&output, root.join("moved")).unwrap();
        fs::create_dir(root.join("outside")).unwrap();
        fs::write(root.join("outside/original.md"), "unchanged").unwrap();
        symlink(root.join("outside"), &output).unwrap();
        write_immutable(&directory, "original.md", b"safe").unwrap();
        assert_eq!(directory.read_regular("original.md").unwrap(), b"safe");
        assert_eq!(fs::read(root.join("moved/original.md")).unwrap(), b"safe");
        assert_eq!(
            fs::read(root.join("outside/original.md")).unwrap(),
            b"unchanged"
        );
        assert!(Directory::open(&root, Path::new("output"), false).is_err());
        // POSIX's `mkfifo` utility: Linux and macOS both ship it.
        let made = Command::new("mkfifo").arg(root.join("moved/fifo")).status();
        assert!(made.unwrap().success());
        assert!(read_without_blocking(&directory, "fifo").is_err());
        assert!(directory.read_regular(".").is_err());
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
        assert!(Directory::open(&root, Path::new("plain"), false).is_err());
        assert!(Directory::open(&root, Path::new("absent/deeper"), false).is_err());
        assert!(!root.join("absent").exists());
        Directory::open(&root, Path::new("absent/deeper"), true).unwrap();
        assert!(root.join("absent/deeper").is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn artifacts_are_never_read_through_a_link() {
        let root = scratch();
        fs::write(root.join("target.md"), "linked").unwrap();
        symlink(root.join("target.md"), root.join("original.md")).unwrap();
        let directory = Directory::open(&root, Path::new(""), false).unwrap();
        assert!(directory.read_regular("original.md").is_err());
        assert_eq!(directory.read_regular("target.md").unwrap(), b"linked");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_directory_in_place_of_an_artifact_is_named_and_kept() {
        let root = scratch();
        fs::create_dir(root.join("original.md")).unwrap();
        let directory = Directory::open(&root, Path::new(""), false).unwrap();
        let error = write_immutable(&directory, "original.md", b"text").unwrap_err();
        assert_eq!(error.to_string(), "artifact is not a regular file");
        assert!(root.join("original.md").is_dir());
        // Windows removes no directory a handle holds open.
        drop(directory);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
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
        let directory = Directory::open(&root, Path::new(""), false).unwrap();
        assert!(write_immutable(&directory, "original.md", b"planted").is_err());
        assert_eq!(fs::read(root.join("victim")).unwrap(), b"unchanged");
        assert!(fs::symlink_metadata(root.join("original.md")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_planted_pending_file_is_never_overwritten() {
        let root = scratch();
        // Other tests may take sequence numbers meanwhile; plant well past the next one.
        let next = TEMP_SEQUENCE.load(Ordering::Relaxed);
        let planted: Vec<PathBuf> = (next..next + 64)
            .map(|sequence| root.join(format!(".original.md.pending-{}-{sequence}", process::id())))
            .collect();
        for pending in &planted {
            fs::write(pending, "unchanged").unwrap();
        }
        let directory = Directory::open(&root, Path::new(""), false).unwrap();
        assert!(write_immutable(&directory, "original.md", b"planted").is_err());
        for pending in &planted {
            assert_eq!(fs::read(pending).unwrap(), b"unchanged");
        }
        assert!(fs::symlink_metadata(root.join("original.md")).is_err());
        drop(directory);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_directory_handle_writes_where_it_was_opened() {
        let root = scratch();
        let directory = Directory::open(&root, Path::new("parent/output"), true).unwrap();
        // Unix lets the ancestor move away; Windows refuses while a directory below it is open.
        let opened = if fs::rename(root.join("parent"), root.join("moved")).is_ok() {
            fs::create_dir_all(root.join("parent/output")).unwrap();
            root.join("moved/output")
        } else {
            root.join("parent/output")
        };
        write_immutable(&directory, "original.md", b"safe").unwrap();
        assert_eq!(fs::read(opened.join("original.md")).unwrap(), b"safe");
        let entries = fs::read_dir(root.join("parent/output")).unwrap().count();
        assert_eq!(entries, usize::from(opened == root.join("parent/output")));
        drop(directory);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn a_verbatim_root_anchors_the_walk_and_parent_traversal_stays_refused() {
        let root = scratch();
        // The root resolves to a verbatim path, such as `\\?\C:\...`, which the walk accepts.
        let resolved = fs::canonicalize(&root).unwrap();
        assert!(matches!(
            resolved.components().next(),
            Some(Component::Prefix(prefix)) if prefix.kind().is_verbatim()
        ));
        drop(Directory::open(&root, Path::new("deeper"), true).unwrap());
        assert!(root.join("deeper").is_dir());
        let error = Directory::open(&root.join("deeper/../deeper"), Path::new(""), false);
        assert_eq!(
            error.unwrap_err().to_string(),
            "snapshot path contains parent traversal"
        );
        for below in ["deeper/../deeper", "C:relative", "C:\\deeper"] {
            let error = Directory::open(&root, Path::new(below), false).unwrap_err();
            assert_eq!(
                error.to_string(),
                "snapshot path below its root holds more than names"
            );
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn a_directory_junction_is_never_followed() {
        let root = scratch();
        fs::create_dir(root.join("outside")).unwrap();
        // A junction, unlike a symbolic link, needs no privilege to create.
        let created = process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(root.join("linked"))
            .arg(root.join("outside"))
            .status()
            .unwrap();
        assert!(created.success());
        assert!(Directory::open(&root, Path::new("linked"), false).is_err());
        assert!(Directory::open(&root, Path::new("linked/deeper"), true).is_err());
        assert!(!root.join("outside/deeper").exists());
        // A root the caller reaches through the junction resolves once and is accepted.
        drop(Directory::open(&root.join("linked"), Path::new(""), false).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_that_cannot_be_created_is_reported_as_such() {
        let root = scratch();
        fs::create_dir(root.join("locked")).unwrap();
        fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o500)).unwrap();
        let error = Directory::open(&root, Path::new("locked/new"), true).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::PermissionDenied);
        fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn of_two_racing_writers_of_different_bytes_one_is_refused() {
        let root = scratch();
        // Both writers usually pass the first check before either links: the loser then meets
        // the winner's file at the link and must compare it, not accept it.
        for round in 0..50 {
            let directory = Directory::open(&root, Path::new(&round.to_string()), true).unwrap();
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
    fn race_writers(directory: &Directory) -> Vec<bool> {
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
