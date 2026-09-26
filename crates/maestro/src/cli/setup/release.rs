//! The Qdrant release `maestro setup` installs, pinned (plan D14, research
//! R7): Qdrant 1.19.1's archive for Linux on x86-64, refused unless its
//! SHA-256 is the one GitHub publishes for it, and the binary it holds,
//! whose SHA-256 is pinned too, so that a second run checks the installed
//! binary without downloading anything. The service binds 127.0.0.1 only.

use std::{error, fmt, path::Path};

/// The address the service listens on: the loopback only.
pub(in crate::cli) const HOST: &str = "127.0.0.1";
/// The port of its HTTP API, which Maestro calls.
pub(in crate::cli) const HTTP_PORT: u16 = 6333;
/// The port of its gRPC API, bound to the loopback too.
pub(super) const GRPC_PORT: u16 = 6334;
/// The service's systemd user unit.
pub(in crate::cli) const SERVICE: &str = "maestro-qdrant.service";

/// A release of Qdrant for one platform: where its archive is, and the
/// SHA-256 of the archive and of the binary it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::cli) struct Release<'a> {
    /// Its version, such as `1.19.1`.
    pub(in crate::cli) version: &'a str,
    /// The URL of its archive, a gzipped tar holding the binary `qdrant`.
    pub(super) archive: &'a str,
    /// The archive's SHA-256, in lowercase hexadecimal.
    pub(super) archive_sha256: &'a str,
    /// The binary's SHA-256, in lowercase hexadecimal.
    pub(super) binary_sha256: &'a str,
}

/// Qdrant 1.19.1 for Linux on x86-64, built against glibc: the archive
/// research R7 measured, whose SHA-256 equals the digest GitHub publishes
/// for the asset (Qdrant publishes no checksum file), and the binary it
/// holds.
pub(in crate::cli) const QDRANT: Release<'static> = Release {
    version: "1.19.1",
    archive: "https://github.com/qdrant/qdrant/releases/download/v1.19.1/\
              qdrant-x86_64-unknown-linux-gnu.tar.gz",
    archive_sha256: "eef986e769d4d3e806dd2d546e1b4ecdd416211e54d34b4ed764fac7c58e1085",
    binary_sha256: "f1823c24376c4a5f2f665d42dc2f52fe4e537e7c2b070b8ae7da575d83327e3a",
};

/// The release setup installs on the platform `os` on `arch`, as
/// `std::env::consts` names them: only Linux on x86-64, the reference
/// workstation's, has one (ADR-0018).
///
/// # Errors
///
/// [`Unsupported`] on any other platform, which gets the manual steps.
pub(in crate::cli) fn release_for(os: &str, arch: &str) -> Result<Release<'static>, Unsupported> {
    if os == "linux" && arch == "x86_64" {
        Ok(QDRANT)
    } else {
        Err(Unsupported {
            os: os.to_owned(),
            arch: arch.to_owned(),
        })
    }
}

/// A platform setup does not install on: Qdrant is set up there by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::cli) struct Unsupported {
    /// The operating system, as `std::env::consts::OS` names it.
    pub(super) os: String,
    /// The architecture, as `std::env::consts::ARCH` names it.
    pub(super) arch: String,
}

impl Unsupported {
    /// The steps that set Qdrant up by hand as setup would, with its
    /// collections in `storage` and its snapshots in `snapshots`, under the
    /// kernel's data directory, started at login as the platform starts a
    /// service.
    pub(in crate::cli) fn manual_steps(&self, storage: &Path, snapshots: &Path) -> String {
        let version = QDRANT.version;
        format!(
            "{self}; set Qdrant {version} up by hand:\n\
             1. download Qdrant {version} for this platform from \
             https://github.com/qdrant/qdrant/releases/tag/v{version} and check it against \
             the SHA-256 GitHub publishes for it;\n\
             2. run it with --disable-telemetry and the variables \
             QDRANT__SERVICE__HOST={HOST}, QDRANT__SERVICE__HTTP_PORT={HTTP_PORT}, \
             QDRANT__SERVICE__GRPC_PORT={GRPC_PORT}, QDRANT__TELEMETRY_DISABLED=true, \
             QDRANT__STORAGE__STORAGE_PATH={} and QDRANT__STORAGE__SNAPSHOTS_PATH={};\n\
             3. start it at login, {};\n\
             4. check it with `maestro doctor`.",
            storage.display(),
            snapshots.display(),
            at_login(&self.os)
        )
    }
}

/// How a service starts at login on the operating system `os`, as
/// `std::env::consts::OS` names it.
fn at_login(os: &str) -> &'static str {
    match os {
        "linux" => "as a systemd user unit, as `maestro setup` writes one on x86_64",
        "macos" => "as a launchd agent",
        "windows" => "as a scheduled task",
        _ => "with the platform's service manager",
    }
}

impl fmt::Display for Unsupported {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "maestro setup installs the search service on Linux on x86_64 with systemd only, \
             and this is {} on {}",
            self.os, self.arch
        )
    }
}

impl error::Error for Unsupported {}
