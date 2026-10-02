//! Preview and apply project files through C04's digest-bound writer.
use super::{
    compose::{Preset, PresetPort, read_preset_file},
    inspect::Inspection,
};
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
    /// Inventory-required binding references, reported as inert data.
    pub bindings: Vec<String>,
    /// Fingerprints of filesystem inputs captured by the inventory adapter.
    sources: Vec<CapturedSource>,
}

/// A captured input fingerprint, never installation or trust authority.
#[derive(Debug)]
struct CapturedSource {
    /// Absolute catalog root captured during resolution.
    root: PathBuf,
    /// Catalog-relative source file.
    path: String,
    /// Digest of the exact bytes decoded at preview.
    sha256: String,
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
    /// Exact checked area selection; no installation or trust authority.
    areas: &'a [String],
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
    /// Exact checked area selection.
    areas: Vec<String>,
    /// Complete selected source closure and its per-resource provenance.
    sources: Vec<LockedSource>,
}

/// One selected input's qualified identity, declared revision and exact digest.
#[derive(Serialize)]
struct LockedSource {
    /// Catalog-relative source path, including sidecars and inventoried assets.
    path: String,
    /// Qualified identity of the owning checked resource.
    id: String,
    /// Declared revision; null for a kind without a declared version.
    revision: Option<String>,
    /// Digest of the captured source bytes.
    sha256: String,
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
    let bindings = presets
        .iter()
        .flat_map(|preset| &preset.bindings)
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let captured = presets
        .iter()
        .flat_map(|preset| {
            preset.source_files.iter().filter_map(|(path, source)| {
                preset.source_root.as_ref().map(|root| CapturedSource {
                    root: root.clone(),
                    path: path.clone(),
                    sha256: digest(&source.bytes),
                })
            })
        })
        .collect();
    let files = project_files(presets, names)?;
    let plan = FilePlan::preview(root, files, trust)
        .map_err(|error| format!("{error}; run preview again; existing bytes were not rebound"))?;
    Ok(BootstrapPreview {
        plan,
        inspection,
        prerequisites,
        bindings,
        sources: captured,
    })
}

/// Emit descriptor and lock from the same resolved inputs used by composition.
fn project_files(presets: Vec<Preset>, names: &[String]) -> Result<Vec<FileInput>, String> {
    let sources = presets
        .iter()
        .flat_map(|preset| preset.source_files.iter())
        .map(|(path, source)| LockedSource {
            path: path.clone(),
            id: source.id.clone(),
            revision: source.revision.clone(),
            sha256: digest(&source.bytes),
        })
        .collect();
    let areas: Vec<_> = presets
        .iter()
        .flat_map(|preset| preset.areas.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut files = super::compose::compose_resolved(presets)?;
    let qualified: Vec<String> = names
        .iter()
        .map(|name| format!("preset:{name}"))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let descriptor = ProjectDescriptor {
        schema: "maestro-project/2",
        lock: ".maestro/authoring.lock.json",
        presets: &qualified,
        areas: &areas,
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
        areas,
        sources,
    };
    let lock_bytes = serde_json::to_vec_pretty(&lock).map_err(|error| error.to_string())?;
    let limit = Limits::PRODUCTION.source_file_bytes;
    if u64::try_from(lock_bytes.len()).map_err(|error| error.to_string())? > limit {
        return Err(format!(
            ".maestro/authoring.lock.json exceeds the {limit}-byte limit"
        ));
    }
    files.push(FileInput::new(".maestro/authoring.lock.json", lock_bytes));
    Ok(files)
}

/// Apply a previously displayed preview through C04's single owned-file writer.
///
/// # Errors
/// Returns a changed-preview or filesystem error without modifying unowned files.
pub fn apply(root: &Path, preview: &BootstrapPreview, trust: &CheckedTrust<'_>) -> io::Result<()> {
    for source in &preview.sources {
        let bytes = read_preset_file(&source.root, &source.path).map_err(|error| {
            io::Error::other(format!("input changed; run preview again: {error}"))
        })?;
        if digest(&bytes) != source.sha256 {
            return Err(io::Error::other("input changed; run preview again"));
        }
    }
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
