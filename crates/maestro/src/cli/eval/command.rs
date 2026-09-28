//! Running the ladder a manifest describes, on this machine.

use super::{
    super::{
        output::{Output, diagnose},
        search::ports,
    },
    comparison::{Comparison, write_comparison},
    engine::KernelEngine,
    graph_ladder,
    graph_manifest::GraphManifest,
    graph_output::Code,
    manifest::Manifest,
    reports::{Binary, RungReport, write_rung},
    runner::run_ladder,
};
use crate::{failure::Failure, kernel::Kernel};
use maestro_knowledge::suite::Suite;
use std::{fs, path::Path, process::ExitCode};

/// Runs the ladder of the manifest at `path`, writing each rung's reports as
/// it ends, then prints the comparison across rungs.
///
/// # Errors
///
/// [`Failure::Refused`] for a manifest or suite that cannot run, before any
/// search; [`Failure`] when the kernel, the services or the output directory
/// fail.
pub(in crate::cli) fn run(output: Output, path: &Path) -> Result<ExitCode, Failure> {
    let manifest = Manifest::read(path).map_err(|error| Code::Manifest.sanitize(&error))?;
    if let Some(graph) = &manifest.graph {
        let inputs = GraphManifest::read(graph).map_err(Code::failure)?;
        return graph_ladder::run(output, &manifest, &inputs);
    }
    run_public(output, &manifest).map_err(|error| Code::Ladder.sanitize(&error))
}

/// Preserves the S1 ladder's report format for manifests without graph inputs.
fn run_public(output: Output, manifest: &Manifest) -> Result<ExitCode, Failure> {
    let text = fs::read_to_string(&manifest.suite)
        .map_err(|error| Failure::refused(format!("cannot read the suite: {error}")))?;
    let suite: Suite = text.parse().map_err(|error| Failure::refused_by(&error))?;
    let kernel = Kernel::open()?;
    let (port, qdrant) = ports()?;
    let mut engine = KernelEngine::new(&kernel, &manifest.collection, port, qdrant)?;
    let binary = Binary::current();
    let runs = run_ladder(
        &mut engine,
        &suite,
        manifest.warm_ups,
        &manifest.rungs,
        |run| {
            let report = RungReport::new(run, &manifest.collection, &suite.digest, binary);
            write_rung(&manifest.output, run, &report)?;
            diagnose(&format!("rung {} written", run.rung.name));
            Ok(())
        },
    )?;
    let comparison = Comparison::new(&runs, &manifest.collection, &suite.digest, binary);
    write_comparison(&manifest.output, &comparison)?;
    output.result(&comparison, &comparison.to_markdown())?;
    Ok(ExitCode::SUCCESS)
}
