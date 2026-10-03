//! Read-only public catalog index/type rendering and exact committed drift checking.

use super::check::today;
use crate::{cli::output::Output, failure::Failure};
use clap::ValueEnum;
use maestro_catalog::{
    limits::Limits,
    source::{
        BY_TYPE_PATH, Directory, INDEX_PATH, Known, Refusal, builtin, frozen_rows, index::generate,
    },
};
use serde::Serialize;
use std::{fs, io, path::Path, process::ExitCode};

/// Which generated public navigation view to print; checking always covers both.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(in crate::cli) enum View {
    /// Versioned public JSON index.
    Index,
    /// Public Markdown navigation grouped by registered type.
    ByType,
}

/// Foreground command document, never source bodies or private diagnostics.
#[derive(Serialize)]
struct Document<'a> {
    /// Stable CLI format version.
    schema: &'static str,
    /// `rendered` or `passed`.
    status: &'static str,
    /// Exact selected view as UTF-8; empty when checking.
    content: &'a str,
}

/// Keep the catalog command's refused/failed exit split.
fn failure(refusal: Refusal) -> Failure {
    if refusal.unreadable() {
        Failure::failed(refusal)
    } else {
        Failure::refused(refusal)
    }
}

/// Render one view or compare both fixed outputs, without writing.
///
/// # Errors
/// Invalid source, unsafe/oversize output, missing views, drift or stdout failure.
pub(in crate::cli) fn run(
    output: Output,
    catalog_dir: &Path,
    view: View,
    check: bool,
) -> Result<ExitCode, Failure> {
    for path in [INDEX_PATH, BY_TYPE_PATH] {
        match fs::symlink_metadata(catalog_dir.join(path)) {
            Ok(metadata) if !metadata.is_file() => {
                return Err(Failure::refused(format!(
                    "{path}: generated output must be a regular file"
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(Failure::failed(format!("{path}: {error}"))),
        }
    }
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().map_err(Failure::failed)?;
    let generated = generate(
        &Directory::new(catalog_dir),
        &builtin().map_err(Failure::failed)?,
        &Limits::PRODUCTION,
        Known {
            rows: &rows,
            settings: &settings,
            today: today()?,
        },
    )
    .map_err(failure)?;
    let content = if check {
        generated.verify_committed().map_err(failure)?;
        ""
    } else {
        match view {
            View::Index => &generated.index,
            View::ByType => &generated.by_type,
        }
    };
    let text = if check {
        "catalog index check passed"
    } else {
        content.strip_suffix('\n').unwrap_or(content)
    };
    output.result(
        &Document {
            schema: "maestro-cli/catalog-index/1",
            status: if check { "passed" } else { "rendered" },
            content,
        },
        text,
    )?;
    Ok(ExitCode::SUCCESS)
}
