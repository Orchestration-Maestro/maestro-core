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
    io::{Read, Seek as _, Write as _},
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
pub(super) fn read(file: &mut File, remaining: &mut u64) -> Result<Vec<u8>, Refusal> {
    if fcntl_getfl(&*file)
        .map_err(|_| Refusal::Configuration)?
        .intersects(WRITABLE)
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
    read_sized(file, length)
}
/// Read once against the observed length; refuse both shrink and growth.
pub(super) fn read_sized(file: &mut impl Read, length: u64) -> Result<Vec<u8>, Refusal> {
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
/// Keep an unsealed image out of concurrent spawns until sealing is complete.
const MEMFD: MemfdFlags = MemfdFlags::CLOEXEC.union(MemfdFlags::ALLOW_SEALING);
/// Both writable open modes must refuse pinned read-only admission.
const WRITABLE: OFlags = OFlags::WRONLY.union(OFlags::RDWR);
/// No mutation or later loosening is possible after image admission.
const IMAGE_SEALS: SealFlags = SealFlags::WRITE
    .union(SealFlags::GROW)
    .union(SealFlags::SHRINK)
    .union(SealFlags::SEAL);
/// Create a private, initially close-on-exec image before writing any bytes.
pub(super) fn image_file() -> Result<File, Refusal> {
    memfd_create(c"maestro-parser-image", MEMFD)
        .map(File::from)
        .map_err(|_| Refusal::Unsupported)
}
/// Seal against writes/growth/shrink, including through any inherited descriptor.
pub(super) fn sealed(bytes: &[u8]) -> Result<File, Refusal> {
    let mut file = image_file()?;
    file.write_all(bytes).map_err(|_| Refusal::Containment)?;
    fcntl_add_seals(&file, IMAGE_SEALS).map_err(|_| Refusal::Unsupported)?;
    fcntl_setfd(&file, FdFlags::empty()).map_err(|_| Refusal::Containment)?;
    Ok(file)
}
/// Installed mode's immutable administrator-owned image metadata.
pub(super) fn installed(file: &File) -> Result<(), Refusal> {
    let meta = file.metadata().map_err(|_| Refusal::LaunchPin)?;
    if !trusted_metadata(meta.is_file(), meta.uid(), meta.mode(), true) {
        return Err(Refusal::LaunchPin);
    }
    Ok(())
}
/// Common immutable administrator-owned metadata; only a loader needs execute bits.
pub(super) fn trusted_metadata(regular: bool, uid: u32, mode: u32, executable: bool) -> bool {
    regular && uid == 0 && mode & 0o022 == 0 && (!executable || mode & 0o111 != 0)
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
pub(super) fn verify_snapshot(path: &Path, digest: Digest, maximum: u64) -> Result<(), Refusal> {
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
    verify: fn(&Path, Digest, u64) -> Result<(), Refusal>,
) -> Result<(File, Option<String>), Refusal> {
    if launch.pids_max < 2 || launch.arguments.iter().any(|arg| arg.contains('\0')) {
        return Err(Refusal::Configuration);
    }
    let parser_bytes = checked(&mut launch.parser, remaining)?;
    let interpreter = super::elf::interpreter(&parser_bytes)?;
    let parser = sealed(&parser_bytes)?;
    write(&root.join("parser"), &parser_bytes, true)?;
    verify(
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
        if !trusted_metadata(meta.is_file(), meta.uid(), meta.mode(), false) {
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
    use std::{
        fs,
        io::{Seek as _, Write as _},
        os::unix::fs::PermissionsExt as _,
        path::Path,
    };

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
        image.rewind().unwrap();
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

#[cfg(test)]
mod preparation_tests {
    use super::{Refusal, prepare, verify_snapshot, write};
    use crate::isolation::elf::interpreter;
    use crate::isolation::{
        port::{RuntimeFile, ScopedRead},
        test_support::{directory, pin, request},
    };
    use rustix::{
        fs::{SealFlags, fcntl_get_seals},
        io::{FdFlags, fcntl_getfd},
    };
    use std::path::Path;
    use std::{
        fs::{self, File},
        os::unix::fs::PermissionsExt as _,
    };

    #[test]
    fn n17_prepare_complete_static_snapshots_inputs_modes_and_staging() {
        let parent = directory();
        let root = parent.join("root");
        fs::create_dir(&root).unwrap();
        let mut launch = request(&parent);
        let input = parent.join("document");
        fs::write(&input, b"scoped bytes").unwrap();
        launch.inputs.push(ScopedRead {
            name: "document".into(),
            file: File::open(&input).unwrap(),
        });
        launch.runtime.push(RuntimeFile {
            path: "etc/fixture".into(),
            pinned: pin(Path::new("/etc/hostname")),
            executable: false,
        });
        let runtime = fs::read("/etc/hostname").unwrap();
        let length = fs::metadata(parent.join("image")).unwrap().len() + 12 + runtime.len() as u64;
        let mut remaining = length;
        let (parser, interpreter) =
            prepare(&root, &mut launch, &mut remaining, verify_snapshot).unwrap();
        assert_eq!(interpreter, None);
        assert_eq!(remaining, 0);
        assert_eq!(
            fs::read(root.join("parser")).unwrap(),
            fs::read(parent.join("image")).unwrap()
        );
        assert_eq!(
            fs::read(root.join("input/document")).unwrap(),
            b"scoped bytes"
        );
        assert_eq!(fs::read(root.join("etc/fixture")).unwrap(), runtime);
        for (name, mode) in [
            ("parser", 0o555),
            ("input/document", 0o444),
            ("etc/fixture", 0o444),
        ] {
            assert_eq!(
                fs::metadata(root.join(name)).unwrap().permissions().mode() & 0o777,
                mode
            );
        }
        assert!(root.join("work").is_dir() && root.join("old-root").is_dir());
        assert_eq!(fcntl_getfd(&parser).unwrap(), FdFlags::empty());
        assert!(
            fcntl_get_seals(&parser)
                .unwrap()
                .contains(SealFlags::WRITE | SealFlags::GROW | SealFlags::SHRINK | SealFlags::SEAL)
        );
        assert_eq!(
            write(&root.join("parser"), b"overwrite", false),
            Err(Refusal::Configuration)
        );
        assert_eq!(
            write(Path::new(""), b"invalid", false),
            Err(Refusal::Configuration)
        );
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn n17_prepare_dynamic_loader_agreement_and_completeness() {
        let parent = directory();
        let mut launch = request(&parent);
        launch.parser = pin(Path::new("/bin/true"));
        let bytes = fs::read("/bin/true").unwrap();
        let loader = interpreter(&bytes).unwrap().unwrap();
        let root = parent.join("missing");
        fs::create_dir(&root).unwrap();
        assert!(matches!(
            prepare(&root, &mut launch, &mut 10_000_000, verify_snapshot),
            Err(Refusal::Configuration)
        ));
        launch.runtime.push(RuntimeFile {
            path: loader.clone(),
            pinned: pin(&Path::new("/").join(&loader)),
            executable: false,
        });
        let root = parent.join("wrong");
        fs::create_dir(&root).unwrap();
        assert!(matches!(
            prepare(&root, &mut launch, &mut 10_000_000, verify_snapshot),
            Err(Refusal::Configuration)
        ));
        launch.runtime.first_mut().unwrap().executable = true;
        let root = parent.join("complete");
        fs::create_dir(&root).unwrap();
        let (_, interpreter) =
            prepare(&root, &mut launch, &mut 10_000_000, verify_snapshot).unwrap();
        assert_eq!(interpreter.as_deref(), Some(loader.as_str()));
        assert_eq!(
            fs::read(root.join(&loader)).unwrap(),
            fs::read(Path::new("/").join(&loader)).unwrap()
        );
        assert_eq!(
            fs::metadata(root.join(loader))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o555
        );
        fs::remove_dir_all(parent).unwrap();
    }
    #[test]
    fn n17_prepare_argument_pid_runtime_and_scoped_name_neighbours() {
        let parent = directory();
        for (index, name) in ["", "../escape", "/absolute", "a/b", ".", ".."]
            .into_iter()
            .enumerate()
        {
            let root = parent.join(format!("input-{index}"));
            fs::create_dir(&root).unwrap();
            let mut launch = request(&parent);
            launch.inputs.push(ScopedRead {
                name: name.into(),
                file: File::open(parent.join("image")).unwrap(),
            });
            assert!(
                matches!(
                    prepare(&root, &mut launch, &mut 1000, verify_snapshot),
                    Err(Refusal::Configuration)
                ),
                "{name}"
            );
        }
        for (index, pids, args) in [(0, 1, vec![]), (1, 2, vec!["bad\0argument".into()])] {
            let root = parent.join(format!("invalid-{index}"));
            fs::create_dir(&root).unwrap();
            let mut launch = request(&parent);
            launch.pids_max = pids;
            launch.arguments = args;
            assert!(matches!(
                prepare(&root, &mut launch, &mut 1000, verify_snapshot),
                Err(Refusal::Configuration)
            ));
        }
        let root = parent.join("unowned-runtime");
        fs::create_dir(&root).unwrap();
        let mut launch = request(&parent);
        launch.runtime.push(RuntimeFile {
            path: "lib/data".into(),
            pinned: pin(&parent.join("image")),
            executable: false,
        });
        assert!(matches!(
            prepare(&root, &mut launch, &mut 1000, verify_snapshot),
            Err(Refusal::LaunchPin)
        ));
        fs::remove_dir_all(parent).unwrap();
    }
}
