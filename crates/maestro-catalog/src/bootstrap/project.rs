//! Preview and apply project files through C04's digest-bound writer.
use super::{compose::PresetPort, inspect::Inspection};
use crate::files::{FileInput, FilePlan, apply as apply_files};
use serde::Serialize;
use std::{fmt::Write as _, io, path::Path};

/// The complete init proposal, including authoring-only descriptor and source lock.
#[derive(Debug)]
pub struct BootstrapPreview {
    /// The C04 plan, whose entries expose their exact bytes and digests.
    pub plan: FilePlan,
    /// Resolved project inspection.
    pub inspection: Inspection,
}

/// The project-local declaration; it contains no authority, policy or hooks.
#[derive(Serialize)]
struct ProjectDescriptor<'a> {
    /// Descriptor schema version.
    schema: &'static str,
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
) -> Result<BootstrapPreview, String> {
    let inspection = super::inspect::inspect(root).map_err(|error| error.to_string())?;
    let presets = port.resolve(names)?;
    let sources = presets
        .iter()
        .flat_map(|preset| preset.source_files.iter())
        .map(|(path, bytes)| LockedFile {
            path: path.clone(),
            sha256: digest(bytes),
        })
        .collect();
    let mut files = super::compose::compose_resolved(presets)?;
    let descriptor = ProjectDescriptor {
        schema: "maestro-project/1",
        presets: names,
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
        schema: "maestro-authoring-lock/1",
        files: locked,
        sources,
    };
    files.push(FileInput::new(
        ".maestro/authoring.lock.json",
        serde_json::to_vec_pretty(&lock).map_err(|error| error.to_string())?,
    ));
    let plan = FilePlan::preview(root, files).map_err(|error| error.to_string())?;
    Ok(BootstrapPreview { plan, inspection })
}

/// Apply a previously displayed preview through C04's single owned-file writer.
///
/// # Errors
/// Returns a changed-preview or filesystem error without modifying unowned files.
pub fn apply(root: &Path, preview: &BootstrapPreview) -> io::Result<()> {
    apply_files(root, &preview.plan)
}

/// SHA-256 digest of bytes using the source lock's stable lowercase form.
fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(&mut hex, "{byte:02x}");
    }
    format!("sha256:{hex}")
}
