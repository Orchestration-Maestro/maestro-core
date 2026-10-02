//! Preview and apply project files through C04's digest-bound writer.
use super::{compose::PresetPort, inspect::Inspection};
use crate::files::{FileInput, FilePlan, apply as apply_files, digest};
use crate::limits::Limits;
use crate::policy::workspace::CheckedTrust;
use maestro_filesystem::Directory;
use serde::Serialize;
use std::{
    collections::BTreeSet,
    env, fs, io,
    path::{Path, PathBuf},
};

/// The complete init proposal, including authoring-only descriptor and source lock.
#[derive(Debug)]
pub struct BootstrapPreview {
    /// The C04 plan, whose entries expose their exact bytes and digests.
    pub plan: FilePlan,
    /// Resolved project inspection.
    pub inspection: Inspection,
    /// Availability of each manifest-declared prerequisite on the current PATH.
    pub prerequisites: Vec<Prerequisite>,
}

/// A manifest-declared tool's availability, checked without executing it.
#[derive(Debug, Serialize)]
pub struct Prerequisite {
    /// Executable name from the preset manifest.
    pub tool: String,
    /// Whether PATH contains an executable for this name.
    pub found: bool,
}

/// The project-local declaration; it contains no authority, policy or hooks.
#[derive(Serialize)]
struct ProjectDescriptor<'a> {
    /// Descriptor schema version.
    schema: &'static str,
    /// Root-relative authoring lock, never installation authority.
    lock: &'static str,
    /// Explicitly selected presets.
    presets: &'a [String],
    /// Declared capabilities, empty until a preset specifies supported data.
    capabilities: &'static [&'static str],
    /// Context files, empty until selected by an explicit preset.
    context_files: &'static [&'static str],
    /// Distinguishes authoring convenience from verified installation.
    mode: &'static str,
}

/// Digest-bound authoring inputs, not trust or installation authority.
#[derive(Serialize)]
struct AuthoringLock {
    /// Lock schema version.
    schema: &'static str,
    /// Every generated file and its SHA-256 digest.
    files: Vec<LockedFile>,
    /// The selected source manifests and templates and their digests.
    sources: Vec<LockedFile>,
}

/// One generated file's root-relative name and digest.
#[derive(Serialize)]
struct LockedFile {
    /// Root-relative file name.
    path: String,
    /// Lowercase SHA-256 digest with algorithm prefix.
    sha256: String,
}

/// Compose preset bytes and preview the full project plan, with no writes.
///
/// # Errors
/// Returns a composition error or an I/O error from inspection or C04 planning.
pub fn preview(
    root: &Path,
    port: &dyn PresetPort,
    names: &[String],
    trust: &CheckedTrust<'_>,
) -> Result<BootstrapPreview, String> {
    if names.is_empty() {
        return Err("select at least one preset".to_owned());
    }
    refuse_old_lock(root)?;
    let inspection = super::inspect::inspect(root).map_err(|error| error.to_string())?;
    let presets = port.resolve(names)?;
    let prerequisites = presets
        .iter()
        .flat_map(|preset| &preset.tools)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|tool| Prerequisite {
            tool: tool.clone(),
            found: tool_found(tool),
        })
        .collect();
    let sources = presets
        .iter()
        .flat_map(|preset| preset.source_files.iter())
        .map(|(path, bytes)| LockedFile {
            path: path.clone(),
            sha256: digest(bytes),
        })
        .collect();
    let mut files = super::compose::compose_resolved(presets)?;
    let qualified: Vec<String> = names.iter().map(|name| format!("preset:{name}")).collect();
    let descriptor = ProjectDescriptor {
        schema: "maestro-project/2",
        lock: ".maestro/authoring.lock.json",
        presets: &qualified,
        capabilities: &[],
        context_files: &[],
        mode: "authoring",
    };
    files.push(FileInput::new(
        ".maestro/project.toml",
        toml::to_string(&descriptor)
            .map_err(|error| error.to_string())?
            .into_bytes(),
    ));
    let locked = files
        .iter()
        .map(|file| LockedFile {
            path: file.path.clone(),
            sha256: digest(&file.bytes),
        })
        .collect();
    let lock = AuthoringLock {
        schema: "maestro-authoring-lock/2",
        files: locked,
        sources,
    };
    files.push(FileInput::new(
        ".maestro/authoring.lock.json",
        serde_json::to_vec_pretty(&lock).map_err(|error| error.to_string())?,
    ));
    let plan = FilePlan::preview(root, files, trust).map_err(|error| error.to_string())?;
    Ok(BootstrapPreview {
        plan,
        inspection,
        prerequisites,
    })
}

/// Apply a previously displayed preview through C04's single owned-file writer.
///
/// # Errors
/// Returns a changed-preview or filesystem error without modifying unowned files.
pub fn apply(root: &Path, preview: &BootstrapPreview, trust: &CheckedTrust<'_>) -> io::Result<()> {
    apply_files(root, &preview.plan, trust)
}

/// Look up executable names on PATH without running any tool or installer.
fn tool_found(tool: &str) -> bool {
    let Some(path) = env::var_os("PATH") else {
        return false;
    };
    let extensions = if cfg!(windows) {
        env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_owned())
    } else {
        String::new()
    };
    let candidates = tool_candidates(tool, &extensions);
    env::split_paths(&path).any(|directory| {
        candidates
            .iter()
            .any(|candidate| is_executable(&directory.join(candidate)))
    })
}

/// Expand Windows PATH extensions; empty extensions preserve Unix's exact-name lookup.
pub(super) fn tool_candidates(tool: &str, extensions: &str) -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from(tool)];
    if Path::new(tool).extension().is_none() {
        candidates.extend(
            extensions
                .split(';')
                .filter(|extension| extension.starts_with('.'))
                .map(|extension| PathBuf::from(format!("{tool}{extension}"))),
        );
    }
    candidates
}

/// A PATH entry must be a file and, on Unix, have an executable permission bit.
fn is_executable(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    // Windows execution uses PATHEXT, not Unix permission bits.
    #[cfg(windows)]
    {
        true
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
}

/// Only the version is needed to reject an old authoring lock before planning.
#[derive(serde::Deserialize)]
struct Envelope {
    /// The lock version, not installation authority.
    schema: String,
}

/// Old authoring inputs cannot silently bind to the /2 source identity cutover.
fn refuse_old_lock(root: &Path) -> Result<(), String> {
    let bytes = match Directory::open(root, Path::new(".maestro"), false).and_then(|directory| {
        directory.read_regular_bounded("authoring.lock.json", Limits::PRODUCTION.source_file_bytes)
    }) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    let envelope: Envelope = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    if envelope.schema != "maestro-authoring-lock/2" {
        return Err(format!(
            "old or unsupported lock {}; maestro-source/2 cutover requires a fresh preview",
            envelope.schema
        ));
    }
    Ok(())
}
