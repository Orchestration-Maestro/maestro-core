//! Reusable private-run approval, resolved-path isolation and explicit kernel open.

use super::{super::filesystem::has_multiple_links, graph_output::Code};
use crate::{failure::Failure, kernel::Kernel};
use maestro_kernel::{artifact::Digest, paths::Environment};
use serde::Deserialize;
use std::{
    env, fs,
    io::ErrorKind,
    net::{IpAddr, SocketAddr},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

/// Explicit bindings: no absent binding ever falls back to the default kernel.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PrivateRun {
    /// XDG data root of the restored scratch kernel.
    scratch_data: PathBuf,
    /// XDG configuration root of the restored scratch kernel.
    scratch_config: PathBuf,
    /// Live XDG data root, excluded even through aliases.
    live_data: PathBuf,
    /// Live XDG configuration root, excluded even through aliases.
    live_config: PathBuf,
    /// Scratch Qdrant's owned storage directory.
    scratch_storage: PathBuf,
    /// Live Qdrant's storage directory.
    live_storage: PathBuf,
    /// Explicit local scratch Qdrant endpoint.
    scratch_endpoint: String,
    /// Live endpoint, never used by the command.
    live_endpoint: String,
    /// Approved root for private inputs and raw diagnostics.
    private_root: PathBuf,
    /// Existing directory below that root for this attempt's output.
    output: PathBuf,
    /// Named restored authoritative backup receipt.
    backup_id: String,
    /// Digest identifying that backup receipt.
    backup_digest: String,
    /// Expiring scope-bound owner approval.
    approval: Approval,
}

/// Owner approval identity; its evidence remains private.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Approval {
    /// Exact collection approved for this run.
    scope: String,
    /// The approved action, graph-evaluation.
    target: String,
    /// Exclusive expiry, seconds since the Unix epoch.
    expires_unix: u64,
    /// Digest of retained approval evidence.
    evidence_digest: String,
}

/// Resolved bindings handed to all private graph commands after validation.
pub(super) struct CheckedRun {
    /// Explicit scratch kernel directory, never the process default.
    data: PathBuf,
    /// Explicit scratch configuration directory.
    config: PathBuf,
    /// Canonical approved private input root.
    root: PathBuf,
    /// Canonical approved private output directory.
    pub(super) output: PathBuf,
}

impl PrivateRun {
    /// Checks approval and all environment/binding overlaps before any kernel open.
    pub(super) fn check(&self, collection: &str) -> Result<CheckedRun, Code> {
        self.check_bindings(
            collection,
            &Environment::current(),
            env::var("MAESTRO_QDRANT_URL").ok().as_deref(),
        )
    }

    /// Pure environment admission seam: tests inject bindings without process-global mutation.
    pub(super) fn check_bindings(
        &self,
        collection: &str,
        environment: &Environment,
        qdrant: Option<&str>,
    ) -> Result<CheckedRun, Code> {
        self.check_approval(collection)?;
        let live = [&self.live_data, &self.live_config, &self.live_storage]
            .map(|path| directory(path))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
        let scratch = [
            &self.scratch_data,
            &self.scratch_config,
            &self.scratch_storage,
        ]
        .map(|path| directory(path))
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;
        for (index, path) in scratch.iter().enumerate() {
            if live
                .iter()
                .chain(scratch.iter().take(index))
                .any(|other| overlaps(path, other))
            {
                return Err(Code::Isolation);
            }
        }
        let data = directory(&self.scratch_data.join("maestro"))?;
        let config = directory(&self.scratch_config.join("maestro"))?;
        if !data.starts_with(directory(&self.scratch_data)?)
            || !config.starts_with(directory(&self.scratch_config)?)
        {
            return Err(Code::Isolation);
        }
        check_children(
            &data,
            &[
                "kernel.sqlite3",
                "kernel.sqlite3-wal",
                "kernel.sqlite3-shm",
                "artifacts",
            ],
        )?;
        check_children(&config, &["config.toml", "bindings.toml"])?;
        self.check_environment(environment, qdrant)?;
        let root = directory(&self.private_root)?;
        let output = directory(&self.output)?;
        if !output.starts_with(&root)
            || live
                .iter()
                .chain(&scratch)
                .any(|path| overlaps(&root, path))
        {
            return Err(Code::Isolation);
        }
        Ok(CheckedRun {
            data,
            config,
            root,
            output,
        })
    }

    /// Refuses absent, expired, wrong-scope or malformed approval identities.
    fn check_approval(&self, collection: &str) -> Result<(), Code> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| Code::Isolation)?
            .as_secs();
        self.check_approval_at(collection, now)
    }

    /// Checks exclusive approval expiry against an injected Unix timestamp.
    pub(super) fn check_approval_at(&self, collection: &str, now: u64) -> Result<(), Code> {
        if self.approval.scope != collection
            || collection.is_empty()
            || self.approval.target != "graph-evaluation"
            || self.approval.expires_unix <= now
            || self.backup_id.is_empty()
            || !self
                .backup_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err(Code::Isolation);
        }
        for digest in [&self.backup_digest, &self.approval.evidence_digest] {
            Digest::parse(digest).map_err(|_| Code::Isolation)?;
        }
        Ok(())
    }

    /// Requires explicit process bindings as well as the receipt; all endpoints are local.
    fn check_environment(
        &self,
        environment: &Environment,
        qdrant: Option<&str>,
    ) -> Result<(), Code> {
        for (value, expected) in [
            (environment.xdg_data_home.as_ref(), &self.scratch_data),
            (environment.xdg_config_home.as_ref(), &self.scratch_config),
        ] {
            let value = value.ok_or(Code::Isolation)?;
            if directory(Path::new(&value))? != directory(expected)? {
                return Err(Code::Isolation);
            }
        }
        let scratch = endpoint(&self.scratch_endpoint)?;
        if scratch == endpoint(&self.live_endpoint)?
            || endpoint_identity(&self.scratch_endpoint)?
                != endpoint_identity(qdrant.ok_or(Code::Isolation)?)?
        {
            return Err(Code::Isolation);
        }
        Ok(())
    }
}

impl CheckedRun {
    /// Only an explicit opener is accepted; callers cannot silently open the default.
    pub(super) fn open_with<T>(
        &self,
        open: impl FnOnce(&Path, &Path) -> Result<T, Failure>,
    ) -> Result<T, Failure> {
        open(&self.data, &self.config)
    }

    /// Opens only the previously checked scratch authority.
    pub(super) fn open(&self) -> Result<Kernel, Failure> {
        self.open_with(Kernel::open_at)
    }

    /// Creates a fresh private artifact; refuses escapes, symlinks and overwrites.
    pub(super) fn write(&self, path: &Path, bytes: &[u8]) -> Result<Digest, Code> {
        let parent = path.parent().ok_or(Code::Isolation)?;
        let parent = directory(parent)?;
        if !parent.starts_with(&self.root) {
            return Err(Code::Isolation);
        }
        let name = path.file_name().ok_or(Code::Isolation)?;
        super::private_write::write(&parent.join(name), |file| {
            use std::io::Write as _;
            file.write_all(bytes)
        })
        .map_err(|_| Code::Output)?;
        Ok(Digest::of(bytes))
    }

    /// Validates an existing output directory inside the approved private root.
    pub(super) fn directory(&self, path: &Path) -> Result<PathBuf, Code> {
        let path = directory(path)?;
        if !path.starts_with(&self.root) {
            return Err(Code::Isolation);
        }
        Ok(path)
    }

    /// Refuses a nonlocal router before any private question can be sent.
    pub(super) fn local_router() -> Result<(), Code> {
        endpoint(&env::var("MAESTRO_ROUTER_URL").map_err(|_| Code::Isolation)?).map(drop)
    }

    /// Resolves an existing input inside the approved private root.
    pub(super) fn input(&self, path: &Path) -> Result<PathBuf, Code> {
        let resolved = fs::canonicalize(path).map_err(|_| Code::Manifest)?;
        if !resolved.starts_with(&self.root) || !resolved.is_file() {
            return Err(Code::Isolation);
        }
        Ok(resolved)
    }
}

/// Canonicalization catches symlinks and parent components on every platform.
fn directory(path: &Path) -> Result<PathBuf, Code> {
    if !path.is_absolute() {
        return Err(Code::Isolation);
    }
    let path = fs::canonicalize(path).map_err(|_| Code::Isolation)?;
    if !path.is_dir() {
        return Err(Code::Isolation);
    }
    Ok(path)
}

/// Either direction matters: a scratch ancestor is as unsafe as a descendant.
fn overlaps(left: &Path, right: &Path) -> bool {
    left.starts_with(right) || right.starts_with(left)
}

/// Local-only endpoint comparison normalizes localhost, IPv4 and IPv6 loopback aliases.
pub(super) fn endpoint(text: &str) -> Result<u16, Code> {
    endpoint_identity(text).map(|socket| socket.port())
}

/// Preserves host identity for process-binding equality, including localhost normalization.
fn endpoint_identity(text: &str) -> Result<SocketAddr, Code> {
    let address = text
        .strip_prefix("http://")
        .ok_or(Code::Isolation)?
        .trim_end_matches('/');
    let normalized = address
        .strip_prefix("localhost:")
        .map_or_else(|| address.to_owned(), |port| format!("127.0.0.1:{port}"));
    let socket: SocketAddr = normalized.parse().map_err(|_| Code::Isolation)?;
    if !IpAddr::is_loopback(&socket.ip()) || socket.port() == 0 {
        return Err(Code::Isolation);
    }
    Ok(socket)
}

/// Existing authority files must not redirect a checked directory into live state.
fn check_children(root: &Path, names: &[&str]) -> Result<(), Code> {
    for name in names {
        match fs::symlink_metadata(root.join(name)) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Err(Code::Isolation),
            Ok(metadata) => {
                if metadata.is_file()
                    && has_multiple_links(&root.join(name), &metadata)
                        .map_err(|_| Code::Isolation)?
                {
                    return Err(Code::Isolation);
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => return Err(Code::Isolation),
        }
    }
    Ok(())
}
