//! Read-once snapshots and sealed native executable handles.
use super::port::{Launch, PinnedFile, Refusal};
use maestro_kernel::artifact::Digest;
use rustix::{
    fs::{MemfdFlags, OFlags, SealFlags, fcntl_add_seals, fcntl_getfl, memfd_create},
    io::{FdFlags, fcntl_setfd},
};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read as _, Seek as _, Write as _},
    os::{
        fd::AsRawFd as _,
        unix::fs::{MetadataExt as _, PermissionsExt as _},
    },
    path::{Component, Path, PathBuf},
};

/// Trusted bootstrap's bounded launch barrier payload; no source-controlled paths.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Configuration {
    /// New, exclusively owned filesystem view.
    pub(crate) root: PathBuf,
    /// Exact sealed parser image inherited from the supervisor.
    pub(crate) parser_fd: i32,
    /// Verified bootstrap image for the single-threaded PID handoff.
    pub(crate) bootstrap_fd: i32,
    /// Explicit parser arguments.
    pub(crate) arguments: Vec<String>,
    /// Exact pinned loader path, if the native image is dynamic.
    pub(crate) interpreter: Option<String>,
    /// Tmpfs scratch ceiling from the same memory envelope.
    pub(crate) memory_bytes: u64,
}
/// File bytes never exceed the cumulative staging envelope.
fn read(file: &mut File, remaining: &mut u64) -> Result<Vec<u8>, Refusal> {
    if fcntl_getfl(&*file)
        .map_err(|_| Refusal::Configuration)?
        .intersects(OFlags::WRONLY | OFlags::RDWR)
    {
        return Err(Refusal::Configuration);
    }
    let metadata = file.metadata().map_err(|_| Refusal::Configuration)?;
    if !metadata.is_file() {
        return Err(Refusal::Configuration);
    }
    let length = metadata.len();
    *remaining = remaining
        .checked_sub(length)
        .ok_or(Refusal::Configuration)?;
    file.rewind().map_err(|_| Refusal::Configuration)?;
    let mut bytes = Vec::new();
    file.take(length.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|_| Refusal::Configuration)?;
    if bytes.len() as u64 != length {
        return Err(Refusal::LaunchPin);
    }
    Ok(bytes)
}
/// A digest pin applies to exactly the buffer passed onward; no second file read.
fn checked(pinned: &mut PinnedFile, remaining: &mut u64) -> Result<Vec<u8>, Refusal> {
    let bytes = read(&mut pinned.file, remaining)?;
    if Digest::of(&bytes) != pinned.digest {
        return Err(Refusal::LaunchPin);
    }
    Ok(bytes)
}
/// Seal against writes/growth/shrink, including through any inherited descriptor.
fn sealed(bytes: &[u8]) -> Result<File, Refusal> {
    let mut file = File::from(
        memfd_create(
            c"maestro-parser-image",
            MemfdFlags::ALLOW_SEALING | MemfdFlags::CLOEXEC,
        )
        .map_err(|_| Refusal::Unsupported)?,
    );
    file.write_all(bytes).map_err(|_| Refusal::Containment)?;
    fcntl_add_seals(
        &file,
        SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL,
    )
    .map_err(|_| Refusal::Unsupported)?;
    fcntl_setfd(&file, FdFlags::empty()).map_err(|_| Refusal::Containment)?;
    Ok(file)
}
/// Installed mode's immutable administrator-owned image metadata.
pub(super) fn installed(file: &File) -> Result<(), Refusal> {
    let meta = file.metadata().map_err(|_| Refusal::LaunchPin)?;
    if !meta.is_file() || meta.uid() != 0 || meta.mode() & 0o022 != 0 || meta.mode() & 0o111 == 0 {
        return Err(Refusal::LaunchPin);
    }
    Ok(())
}
/// Bootstrap pin and explicit host posture, with no auto-detection or fallback.
pub(super) fn bootstrap(
    mut pinned: PinnedFile,
    installed_mode: bool,
    remaining: &mut u64,
) -> Result<File, Refusal> {
    if installed_mode {
        installed(&pinned.file)?;
    }
    let bytes = checked(&mut pinned, remaining)?;
    if installed_mode {
        fcntl_setfd(&pinned.file, FdFlags::empty()).map_err(|_| Refusal::Containment)?;
        return Ok(pinned.file);
    }
    sealed(&bytes)
}
/// Core-written descendant image is digest-checked after writing, before any launch.
fn verify_snapshot(path: &Path, digest: Digest, maximum: u64) -> Result<(), Refusal> {
    let file = File::open(path).map_err(|_| Refusal::LaunchPin)?;
    let mut remaining = maximum;
    checked(&mut PinnedFile { file, digest }, &mut remaining)?;
    Ok(())
}
/// Strict relative paths exclude runtime escape and protected mount collisions.
fn target(root: &Path, path: &str) -> Result<PathBuf, Refusal> {
    if path.is_empty()
        || Path::new(path)
            .components()
            .any(|item| !matches!(item, Component::Normal(_)))
    {
        return Err(Refusal::Configuration);
    }
    let first = Path::new(path)
        .components()
        .next()
        .ok_or(Refusal::Configuration)?;
    if ["input", "work", "parser", "old-root"]
        .iter()
        .any(|name| first.as_os_str() == *name)
    {
        return Err(Refusal::Configuration);
    }
    Ok(root.join(path))
}
/// Create an immutable private snapshot, refusing duplicate paths.
fn write(path: &Path, bytes: &[u8], executable: bool) -> Result<(), Refusal> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| Refusal::Containment)?;
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| Refusal::Configuration)?;
    file.write_all(bytes).map_err(|_| Refusal::Containment)?;
    file.set_permissions(fs::Permissions::from_mode(if executable {
        0o555
    } else {
        0o444
    }))
    .map_err(|_| Refusal::Containment)
}
/// Prepare only explicitly pinned files/scoped handles, never ambient mounts.
pub(super) fn prepare(
    root: &Path,
    launch: &mut Launch,
    remaining: &mut u64,
) -> Result<(File, Option<String>), Refusal> {
    if launch.pids_max < 2 || launch.arguments.iter().any(|arg| arg.contains('\0')) {
        return Err(Refusal::Configuration);
    }
    let parser_bytes = checked(&mut launch.parser, remaining)?;
    let interpreter = super::elf::interpreter(&parser_bytes)?;
    let parser = sealed(&parser_bytes)?;
    write(&root.join("parser"), &parser_bytes, true)?;
    verify_snapshot(
        &root.join("parser"),
        launch.parser.digest.clone(),
        parser_bytes.len() as u64,
    )?;
    for name in ["input", "work", "old-root"] {
        fs::create_dir(root.join(name)).map_err(|_| Refusal::Containment)?;
    }
    let mut loader = interpreter.is_none();
    for runtime in &mut launch.runtime {
        let path = target(root, &runtime.path)?;
        if runtime.executable != (Some(runtime.path.as_str()) == interpreter.as_deref()) {
            return Err(Refusal::Configuration);
        }
        let meta = runtime
            .pinned
            .file
            .metadata()
            .map_err(|_| Refusal::LaunchPin)?;
        if !meta.is_file() || meta.uid() != 0 || meta.mode() & 0o022 != 0 {
            return Err(Refusal::LaunchPin);
        }
        let bytes = checked(&mut runtime.pinned, remaining)?;
        if runtime.executable {
            super::elf::interpreter(&bytes)?;
        }
        write(&path, &bytes, runtime.executable)?;
        loader |= runtime.executable;
    }
    if !loader {
        return Err(Refusal::Configuration);
    }
    for input in &mut launch.inputs {
        if input.name.is_empty()
            || Path::new(&input.name).components().count() != 1
            || !matches!(
                Path::new(&input.name).components().next(),
                Some(Component::Normal(_))
            )
        {
            return Err(Refusal::Configuration);
        }
        let bytes = read(&mut input.file, remaining)?;
        write(&root.join("input").join(&input.name), &bytes, false)?;
    }
    Ok((parser, interpreter))
}
/// Address an inherited pinned image, not a mutable original pathname.
pub(super) fn executable(file: &File) -> PathBuf {
    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
}

#[cfg(test)]
mod tests {
    use super::{
        File, PinnedFile, Refusal, bootstrap, installed, read, sealed, target, verify_snapshot,
    };
    use maestro_kernel::artifact::Digest;
    use maestro_test_scratch::scratch_directory;
    use std::{fs, io::Write as _, os::unix::fs::PermissionsExt as _, path::Path};

    #[test]
    fn n17_pin_seals_owner_permissions_readonly_and_paths() {
        let scratch = scratch_directory().unwrap();
        fs::create_dir_all(&scratch).unwrap();
        let path = scratch.join("synthetic-image");
        fs::write(&path, b"exact bytes").unwrap();
        let wrong = PinnedFile {
            file: File::open(&path).unwrap(),
            digest: Digest::of(b"swapped"),
        };
        assert!(matches!(
            bootstrap(wrong, false, &mut 100),
            Err(Refusal::LaunchPin)
        ));
        let exact = PinnedFile {
            file: File::open(&path).unwrap(),
            digest: Digest::of(b"exact bytes"),
        };
        let mut image = bootstrap(exact, false, &mut 100).unwrap();
        assert!(image.write_all(b"modified").is_err());
        assert!(image.set_len(0).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o777)).unwrap();
        assert_eq!(
            installed(&File::open(&path).unwrap()),
            Err(Refusal::LaunchPin)
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
        assert_eq!(
            installed(&File::open(&path).unwrap()),
            Err(Refusal::LaunchPin)
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o555)).unwrap();
        assert_eq!(
            installed(&File::open(&path).unwrap()),
            Err(Refusal::LaunchPin)
        );
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let mut writable = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert_eq!(read(&mut writable, &mut 100), Err(Refusal::Configuration));
        assert_eq!(
            read(&mut File::open(&path).unwrap(), &mut 9),
            Err(Refusal::Configuration)
        );
        assert_eq!(
            read(&mut File::open("/dev/null").unwrap(), &mut 100),
            Err(Refusal::Configuration)
        );
        for path in [
            "",
            "/absolute",
            "../escape",
            "input/escape",
            "work/evil",
            "parser",
            "old-root/host",
        ] {
            assert_eq!(
                target(Path::new("/owned"), path),
                Err(Refusal::Configuration)
            );
        }
        assert_eq!(
            target(Path::new("/owned"), "lib/library"),
            Ok(Path::new("/owned/lib/library").to_owned())
        );
        drop(sealed(b"pinned").unwrap());
        fs::remove_dir_all(scratch).unwrap();
    }
    #[test]
    fn n17_written_descendant_snapshot_digest_is_checked_before_launch() {
        let scratch = scratch_directory().unwrap();
        fs::create_dir_all(&scratch).unwrap();
        let path = scratch.join("parser");
        fs::write(&path, b"exact bytes").unwrap();
        assert_eq!(
            verify_snapshot(&path, Digest::of(b"exact bytes"), 11),
            Ok(())
        );
        assert_eq!(
            verify_snapshot(&path, Digest::of(b"wrong"), 11),
            Err(Refusal::LaunchPin)
        );
        fs::remove_dir_all(scratch).unwrap();
    }
}
