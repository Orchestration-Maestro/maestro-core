//! `catalog check --catalog-dir DIR`: the strict source checker over a
//! catalog directory, with the built-in kinds, the production limits and the
//! frozen architecture 08 rows and S1's canonical settings registry. It prints each
//! diagnostic with its path and key, and exits 2 when the catalog is refused
//! and 1 when a file or directory cannot be read.

use crate::{cli::output::Output, failure::Failure};
use maestro_catalog::{
    limits::Limits,
    source::{Catalog, Directory, Known, Refusal, builtin, check, frozen_rows},
};
use serde::Serialize;
use std::{
    path::Path,
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

/// The schema of the document `catalog check` prints under `--json`.
const SCHEMA: &str = "maestro-cli/catalog-check/2";

/// What `catalog check` prints under `--json`.
#[derive(Debug, Serialize)]
struct CheckDocument<'a> {
    /// [`SCHEMA`].
    schema: &'static str,
    /// `passed`, `refused`, or `failed` when a file could not be read.
    status: &'static str,
    /// Each checked resource, sorted by ID; none when refused.
    resources: Vec<ResourceLine<'a>>,
    /// Each diagnostic, sorted; none when passed.
    diagnostics: Vec<DiagnosticLine<'a>>,
}

/// One checked resource.
#[derive(Debug, Serialize)]
struct ResourceLine<'a> {
    /// Its qualified ID.
    id: String,
    /// Its primary file, relative to the catalog.
    path: &'a str,
    /// Accountable principals derived from its area.
    owners: Vec<&'a str>,
    /// Delegated content reviewers derived from its area.
    maintainers: Vec<&'a str>,
    /// Its declared stage.
    maturity: &'static str,
}

/// One diagnostic.
#[derive(Debug, Serialize)]
struct DiagnosticLine<'a> {
    /// The file, relative to the catalog; empty for the whole catalog.
    path: &'a str,
    /// The dotted key; empty for the whole file.
    key: &'a str,
    /// What is wrong.
    message: &'a str,
}

/// The document of a passed check.
fn passed(catalog: &Catalog) -> Result<CheckDocument<'_>, Failure> {
    Ok(CheckDocument {
        schema: SCHEMA,
        status: "passed",
        resources: catalog
            .resources
            .iter()
            .map(|resource| {
                let ownership = catalog
                    .ownership(resource)
                    .ok_or_else(|| Failure::failed("checked resource has no area ownership"))?;
                Ok(ResourceLine {
                    id: resource.id.to_string(),
                    path: &resource.path,
                    owners: ownership.owners,
                    maintainers: ownership.maintainers,
                    maturity: resource.metadata.maturity.as_str(),
                })
            })
            .collect::<Result<_, Failure>>()?,
        diagnostics: Vec::new(),
    })
}

/// The document of a refused or failed check.
fn refused(refusal: &Refusal) -> CheckDocument<'_> {
    CheckDocument {
        schema: SCHEMA,
        status: if refusal.unreadable() {
            "failed"
        } else {
            "refused"
        },
        resources: Vec::new(),
        diagnostics: refusal
            .diagnostics
            .iter()
            .map(|diagnostic| DiagnosticLine {
                path: &diagnostic.path,
                key: &diagnostic.key,
                message: &diagnostic.message,
            })
            .collect(),
    }
}

/// Checks the catalog in `catalog_dir`, and prints the result.
///
/// # Errors
///
/// [`Failure::Failed`] when a built-in kind cannot be registered or stdout
/// cannot be written to.
pub(in crate::cli) fn run(output: Output, catalog_dir: &Path) -> Result<ExitCode, Failure> {
    let registry = builtin().map_err(Failure::failed)?;
    let (rows, settings) = (
        frozen_rows(),
        maestro_settings::Registry::built_in().map_err(Failure::failed)?,
    );
    let known = Known {
        rows: &rows,
        settings: &settings,
        today: today()?,
    };
    match check(
        &Directory::new(catalog_dir),
        &registry,
        &Limits::PRODUCTION,
        known,
    ) {
        Ok(catalog) => {
            let text = format!(
                "catalog check passed: {} resources",
                catalog.resources.len()
            );
            output.result(&passed(&catalog)?, &text)?;
            Ok(ExitCode::SUCCESS)
        }
        Err(refusal) => {
            output.refusal(&refused(&refusal), &refusal.to_string())?;
            Ok(ExitCode::from(if refusal.unreadable() { 1 } else { 2 }))
        }
    }
}

/// Supply UTC epoch days at the CLI composition root, never inside catalog checking.
pub(in crate::cli) fn today() -> Result<i64, Failure> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(Failure::failed)?;
    i64::try_from(duration.as_secs() / 86_400).map_err(Failure::failed)
}
