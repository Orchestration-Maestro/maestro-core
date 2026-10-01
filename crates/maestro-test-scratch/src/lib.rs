//! Where the workspace's tests create their scratch directories.
//!
//! The kernel syncs every artifact, its directory and every commit to disk,
//! so on a disk the test suite spends most of its time waiting on `fsync`:
//! the workspace suite took 147 s with its scratch on a disk and 40 s on
//! tmpfs (2026-09-28), and every mutant reruns it. Scratch therefore goes to
//! the RAM-backed `/dev/shm` when it is a directory with room; the durable
//! writes still run, they only return sooner.
//!
//! The base is shared by every user, so each scratch directory is created
//! new under an unpredictable name, owner-only on Unix: a directory or a
//! symlink someone else planted is never used.

#[cfg(unix)]
use rustix::fs::statvfs;
#[cfg(unix)]
use std::os::unix::fs::DirBuilderExt as _;
use std::{
    env,
    ffi::OsString,
    fs::DirBuilder,
    hash::{BuildHasher as _, RandomState},
    io::{self, ErrorKind},
    iter,
    path::{Path, PathBuf},
    process,
    sync::{
        OnceLock,
        atomic::{AtomicU64, Ordering},
    },
    time::SystemTime,
};

#[cfg(test)]
mod tests;

/// The variable that names the scratch base, over every default.
pub const VARIABLE: &str = "MAESTRO_TEST_SCRATCH";

/// The RAM-backed directory preferred when it is a directory with room.
const RAM: &str = "/dev/shm";

/// The free space `RAM` needs to be chosen: 1 GiB.
const ROOM: u64 = 1 << 30;

/// The start of every scratch directory's name, by which a leaked one is
/// found.
const PREFIX: &str = "maestro-tests-";

/// The names tried before creation gives up.
const ATTEMPTS: usize = 16;

/// The base scratch directories are created in: `MAESTRO_TEST_SCRATCH` when
/// it is set, else `/dev/shm` when it is a directory with 1 GiB free, else
/// the platform's temporary directory. It is chosen once per process, so
/// every fixture of a run shares it however free space changes.
///
/// # Errors
///
/// When `MAESTRO_TEST_SCRATCH` is a relative path.
pub fn scratch_root() -> io::Result<PathBuf> {
    static CHOSEN: OnceLock<Result<PathBuf, String>> = OnceLock::new();
    CHOSEN
        .get_or_init(|| {
            base(
                env::var_os(VARIABLE),
                free_space(Path::new(RAM)),
                env::temp_dir(),
            )
            .map_err(|error| error.to_string())
        })
        .clone()
        .map_err(|message| io::Error::new(ErrorKind::InvalidInput, message))
}

/// A new, empty scratch directory under [`scratch_root`], which the caller
/// removes.
///
/// # Errors
///
/// When the base is refused or the directory cannot be created.
pub fn scratch_directory() -> io::Result<PathBuf> {
    create_new(&scratch_root()?)
}

/// A new, empty scratch directory under the platform's temporary directory,
/// which the caller removes: for the programs a test runs, since RAM-backed
/// mounts such as `/dev/shm` are often `noexec`.
///
/// # Errors
///
/// When the directory cannot be created.
pub fn disk_scratch_directory() -> io::Result<PathBuf> {
    create_new(&env::temp_dir())
}

/// A new directory under `base` with an unpredictable name.
fn create_new(base: &Path) -> io::Result<PathBuf> {
    create_in(base, iter::repeat_with(unpredictable_name).take(ATTEMPTS))
}

/// The base `scratch_root` chooses, given the variable's value, the free
/// space of `RAM` and the platform's temporary directory.
fn base(configured: Option<OsString>, ram_free: Option<u64>, temp: PathBuf) -> io::Result<PathBuf> {
    match (configured, ram_free) {
        (Some(path), _) if !path.is_empty() => {
            let path = PathBuf::from(path);
            if path.is_absolute() {
                Ok(path)
            } else {
                Err(io::Error::new(
                    ErrorKind::InvalidInput,
                    format!("{VARIABLE} must be absolute, not {}", path.display()),
                ))
            }
        }
        (_, Some(free)) if free >= ROOM => Ok(PathBuf::from(RAM)),
        _ => Ok(temp),
    }
}

/// The first of `names` created new under `base`, owner-only on Unix. A name
/// already taken, by a directory, a file or a symlink, is skipped.
fn create_in(base: &Path, names: impl IntoIterator<Item = String>) -> io::Result<PathBuf> {
    let builder = owner_only();
    for name in names {
        let path = base.join(name);
        match builder.create(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        ErrorKind::AlreadyExists,
        format!("every scratch name tried is taken under {}", base.display()),
    ))
}

/// A builder of one directory at a time, owner-only on Unix and inheriting
/// its temporary directory's access list on Windows.
fn owner_only() -> DirBuilder {
    let builder = DirBuilder::new();
    #[cfg(unix)]
    let builder = {
        let mut builder = builder;
        builder.mode(0o700);
        builder
    };
    builder
}

/// `PREFIX` and 16 hexadecimal digits another user cannot predict: a hash
/// keyed by the process's random hasher state.
fn unpredictable_name() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let hash =
        RandomState::new().hash_one((process::id(), NEXT.fetch_add(1, Ordering::Relaxed), nanos));
    format!("{PREFIX}{hash:016x}")
}

/// The bytes an unprivileged user may still write under `path`, when it is a
/// directory; none on a platform without `statvfs`.
fn free_space(path: &Path) -> Option<u64> {
    #[cfg(unix)]
    {
        if !path.is_dir() {
            return None;
        }
        let stats = statvfs(path).ok()?;
        Some(stats.f_bavail.saturating_mul(stats.f_frsize))
    }
    #[cfg(not(unix))]
    {
        _ = path;
        None
    }
}
