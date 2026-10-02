//! Graph metrics on the existing ladder; no second execution engine or raw public output.

use super::{
    super::{output::Output, search::ports},
    comparison::{Comparison, write_comparison},
    engine::KernelEngine,
    graph::checked_labels,
    graph_manifest::Inputs,
    graph_output::{self, Code, Counts},
    manifest::Manifest,
    reports::{Binary, RungReport, write_rung},
    runner::{RungRun, run_ladder},
};
use crate::failure::Failure;
use maestro_kernel::artifact::Digest;
use maestro_knowledge::eval::{
    AskOutcome, CheckedLabels, GraphScore, ProofObservation, SearchOutcome, score_graph,
};
use serde_json::json;
use std::{fs, process::ExitCode};

/// Every private failure is retained only under the checked private root.
pub(super) fn run(
    output: Output,
    manifest: &Manifest,
    inputs: &Inputs,
) -> Result<ExitCode, Failure> {
    let result = execute(manifest, inputs);
    match result {
        Ok(counts) => {
            graph_output::emit(output, &inputs.suite.digest, &counts).map_err(Code::failure)?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            graph_output::diagnostic(&inputs.run.output, &error).map_err(Code::failure)?;
            Err(Code::Ladder.sanitize(&error))
        }
    }
}

/// Executes the frozen suite through the one existing runner and keeps each rung.
fn execute(manifest: &Manifest, inputs: &Inputs) -> Result<Counts, Failure> {
    let suite_bytes = fs::read(&manifest.suite).map_err(|_| Code::Manifest.failure())?;
    if Digest::of(&suite_bytes) != inputs.suite.digest {
        return Err(Code::Manifest.failure());
    }
    let labels = checked_labels(inputs).map_err(Code::failure)?;
    let kernel = inputs.run.open()?;
    let (port, qdrant) = ports()?;
    let mut engine = KernelEngine::new(&kernel, &manifest.collection, port, qdrant)?;
    let binary = Binary::current();
    let mut counts = Counts::default();
    let runs = run_ladder(
        &mut engine,
        &inputs.suite,
        manifest.warm_ups,
        &manifest.rungs,
        |run| {
            let report = RungReport::new(run, &manifest.collection, &inputs.suite.digest, binary);
            write_rung(&manifest.output, run, &report)?;
            let score = score_run(run, &labels)?;
            record_counts(&mut counts, run, &labels)?;
            let receipt = json!({"schema":"maestro-graph-proof-score/1", "rung":run.rung.name,
            "labels_digest":inputs.digest.as_str(), "suite_digest":inputs.suite.digest.as_str(),
            "complete":score.final_wire.complete,"of":score.final_wire.of,
            "proof_attrition":{"candidate":score.stages[0], "pre_fusion":score.stages[1],
                "post_packing":score.stages[2], "final_wire":score.stages[3]},
            "conclusions":score.conclusions,
            "families":score.final_wire.families.iter()
            .map(|family|json!({"id":family.family,
            "complete":family.complete})).collect::<Vec<_>>()});
            inputs
                .run
                .write(
                    &manifest
                        .output
                        .join(format!("{}-proofs.json", run.rung.name)),
                    receipt.to_string().as_bytes(),
                )
                .map_err(Code::failure)?;
            Ok(())
        },
    )?;
    let comparison = Comparison::new(&runs, &manifest.collection, &inputs.suite.digest, binary);
    write_comparison(&manifest.output, &comparison)?;
    Ok(counts)
}

/// Failed searches cannot retain credit from a partially populated diagnostic.
pub(super) fn score_run(run: &RungRun, labels: &CheckedLabels) -> Result<GraphScore, Failure> {
    let observations: Vec<_> = run
        .rows
        .iter()
        .zip(&run.diagnostics)
        .map(|(row, diagnostic)| ProofObservation {
            id: row.id.clone(),
            stages: [
                None,
                None,
                None,
                Some(if matches!(row.search.outcome, SearchOutcome::Ranked(_)) {
                    diagnostic.delivered.clone()
                } else {
                    Vec::new()
                }),
            ],
            // G13/G16 supply earlier stages and conclusion validation later.
            conclusions: None,
        })
        .collect();
    score_graph(&labels.items, &observations).map_err(|error| {
        Failure::refused(error.item.map_or_else(
            || error.code.to_owned(),
            |id| format!("{} item={id}", error.code),
        ))
    })
}

/// Accumulates per-rung label and attempted-row totals.
pub(super) fn record_counts(
    counts: &mut Counts,
    run: &RungRun,
    labels: &CheckedLabels,
) -> Result<(), Failure> {
    let score = score_run(run, labels)?;
    counts.items += run.rows.len();
    counts.unreviewed += labels.summary.unreviewed;
    counts.unanswerable += labels.summary.unanswerable;
    counts.links += labels.summary.links;
    counts.anchors += labels.summary.anchors;
    counts.complete += score.final_wire.complete;
    counts.answerable += score.final_wire.of;
    counts.failed += run
        .rows
        .iter()
        .filter(|row| {
            !matches!(row.search.outcome, SearchOutcome::Ranked(_))
                || matches!(row.ask.outcome, AskOutcome::Failed | AskOutcome::TimedOut)
        })
        .count();
    Ok(())
}
