//! Effect-free ownership checks over externally supplied trusted CI evidence.

use super::check::today;
use crate::{cli::output::Output, failure::Failure};
use maestro_catalog::{
    limits::Limits,
    source::{
        Directory, Known, SourceTree, builtin, frozen_rows,
        owners::{OwnerEvidence, OwnerSnapshot, check_owners},
    },
};
use serde::Serialize;
use std::{path::Path, process::ExitCode};

/// A static check result, not a release attestation or runtime admission.
#[derive(Serialize)]
struct Document<'a> {
    /// Versioned command output.
    schema: &'static str,
    /// Successful trusted-input checks.
    status: &'static str,
    /// Exact repository selected by trusted workflow code.
    repository: &'a str,
    /// Exact checked proposed commit.
    head: &'a str,
}

/// Read one bounded, no-follow regular evidence file outside catalog content.
fn evidence(path: &Path) -> Result<OwnerEvidence, Failure> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| Failure::refused("evidence: supply a regular UTF-8-named JSON file"))?;
    let bytes = Directory::new(parent)
        .read(name, Limits::PRODUCTION.source_file_bytes)
        .map_err(|error| Failure::failed(format!("{}: {error}", path.display())))?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > Limits::PRODUCTION.source_file_bytes {
        return Err(Failure::refused(format!(
            "{}: evidence exceeds the source-file byte limit",
            path.display()
        )));
    }
    OwnerEvidence::parse(&bytes)
        .map_err(|error| Failure::refused(format!("{}: {error}", path.display())))
}

/// Check base/head bytes once; no credential lookup or file writes occur here.
/// The trusted workflow must protect the provenance of all inputs and invoke base code.
///
/// # Errors
/// Unsafe/unreadable inputs fail; malformed catalogs or unverifiable evidence refuse.
pub(in crate::cli) fn run(
    output: Output,
    catalog_dir: &Path,
    base_dir: &Path,
    evidence_file: &Path,
    (repository, base_revision, head_revision): (&str, &str, &str),
) -> Result<ExitCode, Failure> {
    let evidence = evidence(evidence_file)?;
    let registry = builtin().map_err(Failure::failed)?;
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().map_err(Failure::failed)?;
    let known = Known {
        rows: &rows,
        settings: &settings,
        today: today()?,
    };
    let snapshot = |revision, directory| {
        OwnerSnapshot::check(
            revision,
            &Directory::new(directory),
            &registry,
            &Limits::PRODUCTION,
            known,
        )
        .map_err(|refusal| {
            if refusal.unreadable() {
                Failure::failed(refusal)
            } else {
                Failure::refused(refusal)
            }
        })
    };
    let base = snapshot(base_revision, base_dir)?;
    let head = snapshot(head_revision, catalog_dir)?;
    check_owners(&base, &head, &evidence, repository).map_err(Failure::refused)?;
    output.result(
        &Document {
            schema: "maestro-cli/catalog-owners/1",
            status: "passed",
            repository,
            head: head_revision,
        },
        "catalog owners passed: trusted evidence matches base ownership and exact head/digests",
    )?;
    Ok(ExitCode::SUCCESS)
}
