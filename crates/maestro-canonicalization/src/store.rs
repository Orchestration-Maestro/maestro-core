//! Immutable snapshots, accessed through directory handles without following symlinks.
use crate::document::CanonicalDocument;
use crate::error::Error;
use crate::hashing::digest;
use crate::replay::validate_document;
use cap_fs_ext::{DirExt, FollowSymlinks, OpenOptionsFollowExt, OpenOptionsSyncExt};
#[cfg(unix)]
use cap_std::fs::{DirBuilderExt, OpenOptionsExt};
use cap_std::{
    ambient_authority,
    fs::{Dir, DirBuilder, File, OpenOptions},
};
use std::{
    ffi::OsStr,
    io::{self, ErrorKind, Read, Write},
    path::{Component, Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A per-process counter that makes each pending file name unique.
static TEMP_SEQUENCE: AtomicUsize = AtomicUsize::new(0);

/// Save exact Markdown and JSON in a versioned local directory on Linux, macOS or Windows.
/// JSON references `original.md` relative to itself. Equal artifacts are reused;
/// existing unequal files, traversal and symlinks are refused, never overwritten.
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

// Directory handles close the check-then-open ancestor/symlink race: Linux and macOS resolve each
// name against an open directory, and Windows cannot rename or delete a directory cap-std holds
// open. The local filesystem must support hard links (ADR-0018).
/// Open a directory one component at a time without following links, creating missing components
/// when asked; parent traversal is refused. The walk starts at the path's root, with its Windows
/// drive or share prefix, or at the working directory for a relative path.
fn open_directory(path: &Path, create: bool) -> io::Result<Dir> {
    if path
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(io::Error::other("snapshot path contains parent traversal"));
    }
    let anchor: PathBuf = path
        .components()
        .take_while(|component| matches!(component, Component::Prefix(_) | Component::RootDir))
        .collect();
    let start = if anchor.as_os_str().is_empty() {
        Path::new(".")
    } else {
        &anchor
    };
    let mut directory = Dir::open_ambient_dir(start, ambient_authority())?;
    for component in path.components() {
        if let Component::Normal(name) = component {
            directory = open_child(&directory, name, create)?;
        }
    }
    Ok(directory)
}

/// Open one child directory without following a link, creating it first when asked and absent.
fn open_child(directory: &Dir, name: &OsStr, create: bool) -> io::Result<Dir> {
    match directory.open_dir_nofollow(name) {
        Err(error) if create && error.kind() == ErrorKind::NotFound => {
            let builder = &mut DirBuilder::new();
            #[cfg(unix)]
            builder.mode(0o700);
            match directory.create_dir_with(name, builder) {
                Ok(()) => sync_directory(directory)?,
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
            directory.open_dir_nofollow(name)
        }
        opened => opened,
    }
}

/// Flush the directory's entries to storage. Windows flushes only through a writable handle and
/// cap-std opens directories read-only, so there NTFS's journal alone makes an entry durable.
fn sync_directory(directory: &Dir) -> io::Result<()> {
    if cfg!(windows) {
        return Ok(());
    }
    directory.open(".")?.sync_all()
}

/// The bytes of a regular file in the directory, never read through a link. The type is checked
/// before the open, which Windows refuses for a directory with an error of its own, and again after
/// it, against a swap in between; the open does not block on a FIFO.
fn read_regular(directory: &Dir, name: &str) -> io::Result<Vec<u8>> {
    let not_regular = || io::Error::other("artifact is not a regular file");
    if !directory.symlink_metadata(name)?.is_file() {
        return Err(not_regular());
    }
    let mut options = OpenOptions::new();
    options.read(true).follow(FollowSymlinks::No).nonblock(true);
    let mut file = directory.open_with(name, &options)?;
    if !file.metadata()?.is_file() {
        return Err(not_regular());
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Whether the file already holds exactly these bytes; different bytes are an error, never
/// overwritten.
fn verify_existing(directory: &Dir, name: &str, bytes: &[u8]) -> io::Result<bool> {
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
fn write_immutable(directory: &Dir, name: &str, bytes: &[u8]) -> io::Result<()> {
    if verify_existing(directory, name, bytes)? {
        return Ok(());
    }
    let temporary = format!(
        ".{name}.pending-{}-{}",
        process::id(),
        TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let mut options = OpenOptions::new();
    options
        .write(true)
        .create_new(true)
        .follow(FollowSymlinks::No);
    #[cfg(unix)]
    options.mode(0o600);
    let file = directory.open_with(&temporary, &options)?;
    let result = publish(directory, file, &temporary, name, bytes);
    let cleanup = directory.remove_file(&temporary);
    result.and(cleanup)
}

/// Fill and sync the pending file, then link it as `name`: a writer that meets another's file
/// there accepts it only when it holds the same bytes.
fn publish(
    directory: &Dir,
    mut file: File,
    temporary: &str,
    name: &str,
    bytes: &[u8],
) -> io::Result<()> {
    file.write_all(bytes)?;
    file.sync_all()?;
    match directory.hard_link(temporary, directory, name) {
        Ok(()) => sync_directory(directory),
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
    use std::{env, fs, sync::Barrier, thread};
    #[cfg(unix)]
    use std::{
        os::unix::fs::{PermissionsExt, symlink},
        process::Command,
        sync::mpsc,
        time::Duration,
    };

    /// A new empty directory; its name does not draw on `TEMP_SEQUENCE`, which pending names use.
    /// macOS reaches the temporary directory through the `/var` link, which the store refuses, so
    /// there the link-free path is used.
    fn scratch() -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let temporary = if cfg!(target_os = "macos") {
            fs::canonicalize(env::temp_dir()).unwrap()
        } else {
            env::temp_dir()
        };
        let root = temporary.join(format!(
            "canonical-store-{}-{}",
            process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        root
    }

    /// `read_regular` on its own thread: the test fails, rather than hangs, if the open blocks.
    #[cfg(unix)]
    fn read_without_blocking(directory: &Dir, name: &'static str) -> io::Result<Vec<u8>> {
        let directory = directory.try_clone().unwrap();
        let (sender, receiver) = mpsc::channel();
        thread::spawn(move || sender.send(read_regular(&directory, name)));
        receiver
            .recv_timeout(Duration::from_secs(5))
            .expect("opening the artifact blocked")
    }

    #[cfg(unix)]
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
        // POSIX's `mkfifo` utility: Linux and macOS both ship it.
        let made = Command::new("mkfifo").arg(root.join("moved/fifo")).status();
        assert!(made.unwrap().success());
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

    #[cfg(unix)]
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
        let directory = open_directory(&root, false).unwrap();
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
        let directory = open_directory(&root, false).unwrap();
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
        let directory = open_directory(&root.join("parent/output"), true).unwrap();
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
    fn a_drive_prefix_anchors_the_walk_and_parent_traversal_stays_refused() {
        let root = scratch();
        assert!(matches!(
            root.components().next(),
            Some(Component::Prefix(_))
        ));
        open_directory(&root.join("deeper"), true).unwrap();
        assert!(root.join("deeper").is_dir());
        let error = open_directory(&root.join("deeper/../deeper"), false).unwrap_err();
        assert_eq!(error.to_string(), "snapshot path contains parent traversal");
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
        assert!(open_directory(&root.join("linked"), false).is_err());
        assert!(open_directory(&root.join("linked/deeper"), true).is_err());
        assert!(!root.join("outside/deeper").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_that_cannot_be_flushed_is_an_error() {
        let root = scratch();
        let directory = open_directory(&root.join("sealed"), true).unwrap();
        // The flush reads the directory, which its owner can no longer do.
        fs::set_permissions(root.join("sealed"), fs::Permissions::from_mode(0o000)).unwrap();
        assert!(sync_directory(&directory).is_err());
        fs::set_permissions(root.join("sealed"), fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_that_cannot_be_created_is_reported_as_such() {
        let root = scratch();
        fs::create_dir(root.join("locked")).unwrap();
        fs::set_permissions(root.join("locked"), fs::Permissions::from_mode(0o500)).unwrap();
        let error = open_directory(&root.join("locked/new"), true).unwrap_err();
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
    fn race_writers(directory: &Dir) -> Vec<bool> {
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
