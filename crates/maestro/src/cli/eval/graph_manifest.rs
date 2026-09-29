//! Versioned, strict manifest shared by inference-free check and private ladder scoring.

use super::{
    graph_output::Code,
    private_run::{CheckedRun, PrivateRun},
};
use maestro_kernel::artifact::Digest;
use maestro_knowledge::{eval::Stage, suite::Suite};
use serde::Deserialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Frozen graph-check inputs; sources come only from the scoped scratch authority.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GraphManifest {
    /// Versioned contract identity.
    schema: String,
    /// Private suite file relative to the manifest.
    suite: PathBuf,
    /// Exact suite digest.
    suite_digest: String,
    /// Private labels file relative to the manifest.
    labels: PathBuf,
    /// Exact labels digest.
    labels_digest: String,
    /// Draft or frozen validation policy.
    stage: Phase,
    /// Scoped authority collection.
    pub(super) collection: String,
    /// Shared private-run guard receipt.
    private_run: PrivateRun,
}

/// Closed validation stage spelling.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    /// Development labels may await review.
    Draft,
    /// Frozen labels require accepted independent review.
    Frozen,
}

/// Loaded and digest-checked private inputs, before any authority open.
pub(super) struct Inputs {
    /// Frozen suite, reused by the existing ladder.
    pub(super) suite: Suite,
    /// Exact labels text; never printed.
    pub(super) labels: String,
    /// Labels digest pinned in the manifest.
    pub(super) digest: Digest,
    /// Review requirement for this attempt.
    pub(super) stage: Stage,
    /// Collection approved for source resolution.
    pub(super) collection: String,
    /// Checked scratch/output bindings.
    pub(super) run: CheckedRun,
}

impl GraphManifest {
    /// Reads strict inputs without opening a kernel or calling any model.
    pub(super) fn read(path: &Path) -> Result<Inputs, Code> {
        let text = fs::read_to_string(path).map_err(|_| Code::Manifest)?;
        let manifest: Self = serde_json::from_str(&text).map_err(|_| Code::Manifest)?;
        if manifest.schema != "maestro-graph-check/1" {
            return Err(Code::Manifest);
        }
        let run = manifest.private_run.check(&manifest.collection)?;
        run.input(path)?;
        let base = path.parent().ok_or(Code::Manifest)?;
        let suite_text = frozen_text(&run, &base.join(manifest.suite), &manifest.suite_digest)?;
        let suite = suite_text.parse().map_err(|_| Code::Manifest)?;
        let labels = frozen_text(&run, &base.join(manifest.labels), &manifest.labels_digest)?;
        let digest = Digest::parse(&manifest.labels_digest).map_err(|_| Code::Manifest)?;
        Ok(Inputs {
            suite,
            labels,
            digest,
            stage: match manifest.stage {
                Phase::Draft => Stage::Draft,
                Phase::Frozen => Stage::Frozen,
            },
            collection: manifest.collection,
            run,
        })
    }
}

/// Digest checking precedes parsing and any open of the scratch authority.
fn frozen_text(run: &CheckedRun, path: &Path, expected: &str) -> Result<String, Code> {
    let digest = Digest::parse(expected).map_err(|_| Code::Manifest)?;
    let text = fs::read_to_string(run.input(path)?).map_err(|_| Code::Manifest)?;
    if Digest::of(text.as_bytes()) != digest {
        return Err(Code::Manifest);
    }
    Ok(text)
}

/// Checks that a private prompt stays inside the approved input root.
pub(super) fn check_prompt_path(inputs: Option<&Inputs>, path: &Path) -> Result<(), Code> {
    if let Some(inputs) = inputs {
        inputs.run.input(path)?;
    }
    Ok(())
}
