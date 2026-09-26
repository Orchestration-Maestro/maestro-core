//! The service setup installs, and the steps that install it: where it
//! lives, the systemd user unit that runs it, what a machine lacks of it,
//! and doing that. Each step checks first, so a second run finds nothing to
//! do and changes nothing.

use super::{
    release::{GRPC_PORT, HOST, HTTP_PORT, Release, SERVICE},
    tools::Tools,
};
use crate::cli::failure::Failure;
use maestro_kernel::artifact::Digest;
use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{self, Write as _},
    path::{Path, PathBuf},
    process,
};

/// The member of the archive that is the binary.
const MEMBER: &str = "qdrant";

/// Where the service lives: its binary and its data under the kernel's data
/// directory, its unit among the user's systemd units.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::cli) struct Layout {
    /// `qdrant` under the kernel's data directory: the service's working
    /// directory, which holds the rest.
    pub(in crate::cli) root: PathBuf,
    /// The binary, `bin/qdrant`.
    pub(in crate::cli) binary: PathBuf,
    /// Its collections, `storage`.
    pub(in crate::cli) storage: PathBuf,
    /// Its snapshots, `snapshots`.
    pub(in crate::cli) snapshots: PathBuf,
    /// The unit, `systemd/user/maestro-qdrant.service` under the
    /// configuration home, where systemd's user manager reads it.
    pub(in crate::cli) unit: PathBuf,
}

impl Layout {
    /// The layout of the kernel whose data directory is `data`, under the
    /// configuration home `config_home`, the directory the kernel's own
    /// configuration directory is in.
    pub(in crate::cli) fn new(data: &Path, config_home: &Path) -> Self {
        let root = data.join("qdrant");
        Self {
            binary: root.join("bin").join("qdrant"),
            storage: root.join("storage"),
            snapshots: root.join("snapshots"),
            unit: config_home.join("systemd").join("user").join(SERVICE),
            root,
        }
    }
}

/// A step of the install, in the order they run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::cli) enum Step {
    /// Download the archive, check it and the binary it holds against their
    /// pins, and install the binary.
    Install,
    /// Write the unit.
    WriteUnit,
    /// Have systemd's user manager read the unit again.
    Reload,
    /// Enable the service, which starts it at login.
    Enable,
    /// Start the service, or restart it on what changed.
    Restart,
}

/// The unit that runs `release` as `layout` places it: bound to the loopback
/// address, telemetry off, its storage and snapshots under the kernel's
/// data directory. Each path is written as systemd reads it back, with `%`
/// doubled, since systemd expands specifiers everywhere; a `$` stays as it
/// is, since systemd expands no variable in an executable's path, in
/// `WorkingDirectory=` or in `Environment=`.
///
/// # Errors
///
/// [`Failure::Refused`] naming a path that no unit can hold.
pub(super) fn unit_text(layout: &Layout, release: &Release<'_>) -> Result<String, Failure> {
    let root = unit_path(&layout.root)?.replace('%', "%%");
    let binary = quoted(unit_path(&layout.binary)?);
    let storage = unit_path(&layout.storage)?;
    let snapshots = unit_path(&layout.snapshots)?;
    let mut lines = vec![
        "# Written by `maestro setup`, which writes it again whenever it differs.".to_owned(),
        "[Unit]".to_owned(),
        format!(
            "Description=Qdrant {}, the search service of Maestro",
            release.version
        ),
        String::new(),
        "[Service]".to_owned(),
        "Type=simple".to_owned(),
        format!("WorkingDirectory={root}"),
        format!("ExecStart={binary} --disable-telemetry"),
    ];
    lines.extend(
        [
            ("QDRANT__SERVICE__HOST", HOST),
            ("QDRANT__SERVICE__HTTP_PORT", &HTTP_PORT.to_string()),
            ("QDRANT__SERVICE__GRPC_PORT", &GRPC_PORT.to_string()),
            ("QDRANT__STORAGE__STORAGE_PATH", storage),
            ("QDRANT__STORAGE__SNAPSHOTS_PATH", snapshots),
            ("QDRANT__TELEMETRY_DISABLED", "true"),
        ]
        .iter()
        .map(|(name, value)| format!("Environment={}", quoted(&format!("{name}={value}")))),
    );
    lines.extend(
        [
            "Restart=on-failure",
            "",
            "[Install]",
            "WantedBy=default.target",
            "",
        ]
        .map(str::to_owned),
    );
    Ok(lines.join("\n"))
}

/// `path` as a unit's text, when a unit can hold it: UTF-8 without a
/// control character, a quote or a backslash, which systemd refuses in the
/// path of an executable.
fn unit_path(path: &Path) -> Result<&str, Failure> {
    path.to_str()
        .filter(|text| {
            !text
                .chars()
                .any(|character| character.is_control() || "\"'\\".contains(character))
        })
        .ok_or_else(|| {
            Failure::refused(format!(
                "a systemd unit cannot hold the path {:?}: it is no UTF-8, or holds a \
                 control character, a quote or a backslash; set XDG_DATA_HOME to a plainer \
                 directory",
                path.display()
            ))
        })
}

/// `text` in double quotes, as `ExecStart=` and `Environment=` read a word
/// that holds spaces, with `%` doubled.
fn quoted(text: &str) -> String {
    format!("\"{}\"", text.replace('%', "%%"))
}

/// Refuses a machine where no systemd user manager runs, before any step:
/// setup runs the search service as a user unit, which only a user manager
/// starts, and `systemctl --user show-environment` fails without one. On
/// WSL, systemd runs only when `/etc/wsl.conf` sets `systemd=true` under
/// `[boot]`.
///
/// # Errors
///
/// [`Failure::Refused`] naming what `systemctl` said and that setting, and
/// [`Failure::Failed`] when `systemctl` cannot run at all.
pub(in crate::cli) fn user_manager(tools: &Tools) -> Result<(), Failure> {
    let answer = tools.systemctl(&["show-environment"])?;
    if answer.success {
        return Ok(());
    }
    Err(Failure::refused(format!(
        "no systemd user manager runs for this user ({}), and setup runs the search service \
         as a systemd user unit: start one, then run setup again; on WSL, set \
         `systemd=true` under `[boot]` in /etc/wsl.conf, then restart WSL with \
         `wsl --shutdown`",
        answer.stderr
    )))
}

/// What the machine lacks of `release` as `layout` places it, in the order
/// the steps run; none when everything is in place. It only reads, and asks
/// the user manager whether the service is enabled and running.
///
/// # Errors
///
/// [`Failure::Refused`] when a path cannot be written in a unit, and
/// [`Failure::Failed`] when `systemctl` cannot run.
pub(in crate::cli) fn survey(
    layout: &Layout,
    release: &Release<'_>,
    tools: &Tools,
) -> Result<Vec<Step>, Failure> {
    let unit = unit_text(layout, release)?;
    let binary_in_place = installed(&layout.binary, release.binary_sha256);
    let unit_in_place = fs::read(&layout.unit).is_ok_and(|written| written == unit.as_bytes());
    let enabled = tools.systemctl(&["is-enabled", SERVICE])?.stdout == "enabled";
    let active = tools.systemctl(&["is-active", SERVICE])?.stdout == "active";
    let mut steps = Vec::new();
    if !binary_in_place {
        steps.push(Step::Install);
    }
    if !unit_in_place {
        steps.extend([Step::WriteUnit, Step::Reload]);
    }
    if !enabled {
        steps.push(Step::Enable);
    }
    if !(binary_in_place && unit_in_place && active) {
        steps.push(Step::Restart);
    }
    Ok(steps)
}

/// Whether the file at `path` is the binary pinned by `sha256`, and, on
/// Unix, its owner may run it.
fn installed(path: &Path, sha256: &str) -> bool {
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        if !fs::metadata(path).is_ok_and(|metadata| metadata.permissions().mode() & 0o100 != 0) {
            return false;
        }
    }
    Digest::of(&bytes).as_str() == sha256
}

/// Takes `steps`, in their order, to install `release` as `layout` places
/// it. The install downloads and checks both digests before it writes
/// anything, and each file is written whole or not at all.
///
/// # Errors
///
/// [`Failure::Failed`] naming the step that failed and why: a download
/// that is not the pinned one, a file that cannot be written or a
/// `systemctl` that fails. The steps before it stay done, and a rerun takes
/// the rest.
pub(super) fn apply(
    steps: &[Step],
    layout: &Layout,
    release: &Release<'_>,
    tools: &Tools,
) -> Result<(), Failure> {
    for step in steps {
        match step {
            Step::Install => install(layout, release, tools)?,
            Step::WriteUnit => {
                write_whole(&layout.unit, unit_text(layout, release)?.as_bytes(), 0o600)?;
            }
            Step::Reload => tools.systemctl_must(&["daemon-reload"])?,
            Step::Enable => tools.systemctl_must(&["enable", SERVICE])?,
            Step::Restart => tools.systemctl_must(&["restart", SERVICE])?,
        }
    }
    Ok(())
}

/// Downloads the archive of `release`, checks it and the binary it holds
/// against their pins, then installs the binary where `layout` places it.
fn install(layout: &Layout, release: &Release<'_>, tools: &Tools) -> Result<(), Failure> {
    let archive = tools.fetch(release.archive)?;
    pinned(&archive, release.archive_sha256, release.archive)?;
    let binary = tools.unpack(&archive, MEMBER)?;
    pinned(&binary, release.binary_sha256, "the binary it holds")?;
    write_whole(&layout.binary, &binary, 0o700)
}

/// Refuses `bytes` unless their SHA-256 is `sha256`, the pin of `what`.
fn pinned(bytes: &[u8], sha256: &str, what: &str) -> Result<(), Failure> {
    let found = Digest::of(bytes);
    if found.as_str() == sha256 {
        Ok(())
    } else {
        Err(Failure::failed(format!(
            "refused {what}: its SHA-256 is {}, not the pinned {sha256}; nothing was written",
            found.as_str()
        )))
    }
}

/// Writes `bytes` to `path` whole or not at all: to a new file beside it,
/// flushed, then renamed into place. On Unix the file has the permission
/// bits `mode`, and the directories it lacks are created for the owner
/// only; setup installs on Linux alone (ADR-0018).
fn write_whole(path: &Path, bytes: &[u8], mode: u32) -> Result<(), Failure> {
    let cannot =
        |error: io::Error| Failure::failed(format!("cannot write {}: {error}", path.display()));
    let directory = path.parent().unwrap_or(path);
    let mut directories = DirBuilder::new();
    let mut options = OpenOptions::new();
    directories.recursive(true);
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};
        directories.mode(0o700);
        options.mode(mode);
    }
    #[cfg(not(unix))]
    let _ = mode;
    directories.create(directory).map_err(cannot)?;
    let temporary = directory.join(format!(".setup-{}.tmp", process::id()));
    let written = options
        .open(&temporary)
        .and_then(|mut file: File| {
            file.write_all(bytes)?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&temporary, path));
    if written.is_err() {
        drop(fs::remove_file(&temporary));
    }
    written.map_err(cannot)
}
