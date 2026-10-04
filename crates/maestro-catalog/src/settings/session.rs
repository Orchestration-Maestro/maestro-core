//! Frozen defaults admitted by trust and the lock's own committed C04 ownership.
use super::{
    admission_error::AdmissionError,
    defaults::manifest_registry,
    discovery::{SessionPreferences, recovery},
};
use crate::{
    files::{FilePlan, digest},
    limits::Limits,
    policy::workspace::{Access, CheckedTrust, WorkspaceTrust as _},
};
use maestro_settings::Registry;
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    io::{self, Read as _},
    path::Path,
};

/// Captured defaults and authoring outputs; neither confers installation authority.
#[derive(Deserialize)]
struct Lock {
    /// Lowest slot, independent of user/workspace overrides.
    defaults: String,
    /// Base adapters required by the authoring plan.
    backend_types: BTreeSet<String>,
    /// Generated outputs used only for drift notes and bounds.
    files: Vec<LockedFile>,
}

/// One original generated output.
#[derive(Deserialize)]
struct LockedFile {
    /// Root-relative output path.
    path: String,
    /// Original byte digest.
    sha256: String,
}

impl SessionPreferences {
    /// Admit the lock's defaults after trust, safe bounded reads and own C04 ownership.
    /// Output drift is a note; catalogs are never reread during a session.
    /// Verification is read-only, including refusals; health may use it non-fatally.
    ///
    /// # Errors
    /// Refuses untrusted, changed, malformed, oversized or uncommitted locks,
    /// unsupported schemas and unavailable base adapters.
    pub fn admit_defaults(
        mut self,
        trust: &CheckedTrust<'_>,
        compiled: &BTreeSet<String>,
        limits: &Limits,
    ) -> Result<Self, AdmissionError> {
        let Some(path) = self.lock.take().transpose()? else {
            return Ok(self);
        };
        let root = path
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| recovery(&path, "invalid lock placement"))?;
        if trust.containing_root(root).is_none() {
            return Err(
                recovery(&path, "project defaults require a trusted containing root").into(),
            );
        }
        let bytes = trust
            .authorize(
                root,
                Path::new(".maestro/authoring.lock.json"),
                Access::Read,
            )
            .and_then(|path| path.read_preferences(limits.source_file_bytes))
            .map_err(|error| recovery(&path, format!("project lock cannot be read: {error}")))?;
        // Decode the version alone so an old /2 shape refuses as unsupported, not invalid.
        let envelope: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| recovery(&path, "invalid project lock"))?;
        if envelope.get("schema").and_then(serde_json::Value::as_str)
            != Some("maestro-authoring-lock/3")
        {
            return Err(recovery(&path, "unsupported project lock").into());
        }
        let lock: Lock = serde_json::from_value(envelope)
            .map_err(|_| recovery(&path, "invalid project lock"))?;
        let mut total = u64::try_from(bytes.len()).map_err(|error| recovery(&path, error))?;
        if lock.files.len() >= limits.archive_entries {
            return Err(recovery(&path, "project lock output count exceeds limit").into());
        }
        if total > limits.archive_total_bytes {
            return Err(recovery(&path, "project lock output bytes exceed limit").into());
        }
        let owned =
            FilePlan::committed_file(root, ".maestro/authoring.lock.json", &bytes, trust, limits)
                .map_err(|error| recovery(&path, error))?;
        let drift = output_drift(root, &lock.files, &owned, trust, (limits, &mut total))
            .map_err(|error| recovery(&path, error))?;
        if !drift.is_empty() {
            let note = format!(
                "generated outputs drifted: {}; maestro init regenerates them",
                drift.join(", ")
            );
            self.discovery.note = Some(
                self.discovery
                    .note
                    .map_or(note.clone(), |prior| format!("{prior}; {note}")),
            );
        }
        if let Some(backend) = lock
            .backend_types
            .iter()
            .find(|kind| *kind != "none" && !compiled.contains(*kind))
        {
            return Err(AdmissionError::BackendNotCompiled {
                backend: backend.clone(),
                message: recovery(&path, "backend adapter is not compiled into this build"),
            });
        }
        self.registry = manifest_registry(
            &Registry::built_in().map_err(|error| recovery(&path, error))?,
            &[(".maestro/authoring.lock.json".to_owned(), lock.defaults)],
            limits,
        )
        .map_err(|(_, key, message)| recovery(&path, format!("{key}: {message}")))?;
        self.frozen_lock = Some(digest(&bytes));
        Ok(self)
    }
}

/// Output content and ownership drift never confer authority or refuse admission.
fn output_drift(
    root: &Path,
    entries: &[LockedFile],
    owned: &[(String, String)],
    trust: &CheckedTrust<'_>,
    (limits, total): (&Limits, &mut u64),
) -> io::Result<Vec<String>> {
    let mut drift = Vec::new();
    for entry in entries {
        let bytes = match read(root, &entry.path, trust, limits.source_file_bytes) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::FileTooLarge => {
                return Err(io::Error::new(
                    error.kind(),
                    format!("{}: {error}", entry.path),
                ));
            }
            Err(_) => {
                drift.push(entry.path.clone());
                continue;
            }
        };
        *total += u64::try_from(bytes.len()).map_err(io::Error::other)?;
        if *total > limits.archive_total_bytes {
            return Err(io::Error::other(format!(
                "{}: project lock output bytes exceed limit",
                entry.path
            )));
        }
        if digest(&bytes) != entry.sha256
            || !owned.contains(&(entry.path.clone(), entry.sha256.clone()))
        {
            drift.push(entry.path.clone());
        }
    }
    if *total > limits.archive_total_bytes {
        return Err(io::Error::other("project lock output bytes exceed limit"));
    }
    Ok(drift)
}

/// Held-handle non-secret output reads keep the source-file byte ceiling.
fn read(root: &Path, relative: &str, trust: &CheckedTrust<'_>, max: u64) -> io::Result<Vec<u8>> {
    let file = trust
        .authorize(root, Path::new(relative), Access::Read)?
        .open_read()?;
    let mut bytes = Vec::new();
    file.take(max.saturating_add(1)).read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max {
        return Err(io::Error::new(
            io::ErrorKind::FileTooLarge,
            "project output exceeds byte limit",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::read;
    use crate::files::tests::support::with_trust;
    use maestro_test_scratch::scratch_directory;
    use std::{fs, io};

    #[test]
    fn output_read_accepts_exact_limit_and_refuses_one_more_byte() {
        let root = scratch_directory().unwrap();
        fs::write(root.join("output"), b"abc").unwrap();
        with_trust(&root, |trust| {
            assert_eq!(read(&root, "output", trust, 3).unwrap(), b"abc");
            assert_eq!(
                read(&root, "output", trust, 2).unwrap_err().kind(),
                io::ErrorKind::FileTooLarge
            );
        });
        fs::remove_dir_all(root).unwrap();
    }
}
