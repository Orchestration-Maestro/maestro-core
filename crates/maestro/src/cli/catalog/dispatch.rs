//! Catalog-only dispatch; foreground session and repair ordering stay at the caller.

use super::{check, codeowners, owners};
use crate::{
    cli::{args::CatalogCommand, output::Output},
    failure::Failure,
};
use std::process::ExitCode;

/// Route catalog commands without changing their checked inputs or output.
///
/// # Errors
/// The selected foreground command's source/evidence/output refusal.
pub(in crate::cli) fn run(output: Output, command: &CatalogCommand) -> Result<ExitCode, Failure> {
    match command {
        CatalogCommand::Owners {
            catalog_dir,
            base_dir,
            evidence,
            repository,
            base_revision,
            head_revision,
            ..
        } => owners::run(
            output,
            catalog_dir,
            base_dir,
            evidence,
            (repository, base_revision, head_revision),
        ),
        CatalogCommand::Check { catalog_dir } => check::run(output, catalog_dir),
        CatalogCommand::Codeowners { catalog_dir, check } => {
            codeowners::run(output, catalog_dir, *check)
        }
    }
}
