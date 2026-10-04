//! `catalog compile`: one inert snapshot, then exclusive verified publication.

use super::check::today;
use crate::{cli::output::Output, failure::Failure};
use clap::Args;
use maestro_catalog::{
    bundle::{Bundle, compile},
    limits::Limits,
    source::{Directory as SourceDirectory, Known, builtin, frozen_rows},
};
use maestro_filesystem::{Directory, PublicationChecks};
use serde::Serialize;
use std::{
    io::{self, Write as _},
    path::{Path, PathBuf},
    process::{self, ExitCode},
};

/// Compile arguments live with their implementation, not the shared dispatch.
#[derive(Debug, Args)]
pub(in crate::cli) struct Arguments {
    /// The catalog source directory.
    #[arg(long, value_name = "DIR")]
    catalog_dir: PathBuf,
    /// A new bundle path; existing outputs are never replaced.
    #[arg(long, value_name = "FILE")]
    output: PathBuf,
    /// Full lowercase 40- or 64-character source commit, supplied by release CI.
    #[arg(long, value_name = "SHA")]
    source_commit: String,
}

/// The digest of the exact published bytes, not of a reserialized manifest.
#[derive(Serialize)]
struct Compiled<'a> {
    /// Versioned CLI output envelope.
    schema: &'static str,
    /// Successful exclusive publication.
    status: &'static str,
    /// Exact archive digest.
    digest: &'a str,
    /// Published archive bytes.
    bytes: usize,
}

/// Check all inputs and limits before creating any output.
///
/// # Errors
/// A source/limit refusal or filesystem/output failure.
pub(in crate::cli) fn run(output: Output, args: &Arguments) -> Result<ExitCode, Failure> {
    let registry = builtin().map_err(Failure::failed)?;
    let rows = frozen_rows();
    let settings = maestro_settings::Registry::built_in().map_err(Failure::failed)?;
    let bundle = compile(
        &SourceDirectory::new(&args.catalog_dir),
        &registry,
        &Limits::PRODUCTION,
        Known {
            rows: &rows,
            settings: &settings,
            today: today()?,
        },
        &args.source_commit,
    )
    .map_err(|refusal| {
        if refusal.unreadable() {
            Failure::failed(refusal)
        } else {
            Failure::refused(refusal)
        }
    })?;
    publish(&args.output, &bundle).map_err(Failure::failed)?;
    output.result(
        &Compiled {
            schema: "maestro-cli/catalog-compile/1",
            status: "compiled",
            digest: &bundle.digest,
            bytes: bundle.bytes.len(),
        },
        &format!("catalog compile {}", bundle.digest),
    )?;
    Ok(ExitCode::SUCCESS)
}

/// Stage a complete sibling and publish with the existing held-handle adapter.
/// Temporary cleanup is identity checked, including on publication failure.
fn publish(path: &Path, bundle: &Bundle) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| io::Error::other("output needs a UTF-8 filename"))?;
    let directory = Directory::open(parent, Path::new(""), false)?;
    let temporary = format!(".maestro-bundle-{}.tmp", process::id());
    let mut file = directory.create_new(&temporary)?;
    let staged = file.write_all(&bundle.bytes).and_then(|()| file.sync_all());
    let result = staged.and_then(|()| {
        directory.publish_verified(
            &temporary,
            name,
            &bundle.bytes,
            PublicationChecks {
                after_source_open: || Ok(()),
                before_link: || Ok(()),
                after_link: || Ok(()),
            },
        )
    });
    // A partial staging write is cleaned against the actual bytes on its held handle.
    let length = file.metadata()?.len();
    let expected = bundle
        .bytes
        .get(..usize::try_from(length).map_err(io::Error::other)?)
        .ok_or_else(|| io::Error::other("staging bytes changed; refusing cleanup"))?;
    let cleanup = directory.remove_created_bytes(&temporary, &file, expected);
    cleanup?;
    result
}
