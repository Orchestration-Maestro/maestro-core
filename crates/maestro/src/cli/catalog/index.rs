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

#[cfg(test)]
mod tests {
    use super::{BY_TYPE_PATH, Failure, INDEX_PATH, Output, View, run};
    use maestro_test_scratch::scratch_directory;
    #[cfg(unix)]
    use std::io::ErrorKind;
    use std::{fs, path::PathBuf};

    /// Minimal public source, sufficient to distinguish preflight from checking.
    fn catalog() -> PathBuf {
        let root = scratch_directory().unwrap();
        fs::create_dir(root.join("core")).unwrap();
        for (path, text) in [
            (
                "package.toml",
                include_str!("../../../../../tests/fixtures/catalog/source/package.toml"),
            ),
            (
                "core/package.toml",
                include_str!("../../../../../tests/fixtures/catalog/source/core-package.toml"),
            ),
        ] {
            fs::write(root.join(path), text).unwrap();
        }
        root
    }

    #[test]
    fn catalog_index_directories_refuse_at_preflight() {
        let root = catalog();
        // Missing outputs are valid for rendering; regular files are valid too.
        run(Output::new(false), &root, View::Index, false).unwrap();
        let package = root.join("core/package.toml");
        let valid = fs::read(&package).unwrap();
        for path in [INDEX_PATH, BY_TYPE_PATH] {
            fs::create_dir_all(root.join(path).parent().unwrap()).unwrap();
            fs::write(root.join(path), "stale\n").unwrap();
            run(Output::new(false), &root, View::Index, false).unwrap();
            fs::remove_file(root.join(path)).unwrap();
            fs::create_dir(root.join(path)).unwrap();
            // Preflight must refuse the output before parsing malformed sources;
            // a valid source alone would reach the same refusal in the snapshot.
            fs::write(&package, "unknown = true\n").unwrap();
            for check in [false, true] {
                let error = run(Output::new(false), &root, View::Index, check).unwrap_err();
                assert!(matches!(error, Failure::Refused(_)));
                assert_eq!(
                    error.to_string(),
                    format!("{path}: generated output must be a regular file")
                );
            }
            fs::remove_dir(root.join(path)).unwrap();
            fs::write(&package, &valid).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }

    // Unix reports NotADirectory here; Windows may report NotFound instead.
    // The directory test above keeps missing/regular-file neighbours portable.
    #[cfg(unix)]
    #[test]
    fn catalog_index_non_not_found_io_fails_at_preflight() {
        let root = catalog();
        run(Output::new(false), &root, View::Index, false).unwrap();
        // A regular ancestor distinguishes a missing output from an I/O failure.
        for (path, ancestor) in [(INDEX_PATH, "marketplace"), (BY_TYPE_PATH, "docs")] {
            fs::write(root.join(ancestor), "not a directory\n").unwrap();
            let io_error = fs::symlink_metadata(root.join(path)).unwrap_err();
            assert_ne!(io_error.kind(), ErrorKind::NotFound);
            for check in [false, true] {
                let error = run(Output::new(false), &root, View::Index, check).unwrap_err();
                assert!(matches!(error, Failure::Failed(_)), "{error:?}");
                assert_eq!(error.to_string(), format!("{path}: {io_error}"));
            }
            fs::remove_file(root.join(ancestor)).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }
}
