//! Reranker selection from a ladder rung's report and its manifest.
//!
//! The helper refuses evidence that does not measure the card on the
//! published generation, then records the evaluation and the selection.

use crate::{
    artifact::Digest,
    gateway::Role,
    model::{
        EvaluationDisposition, EvaluationMode, NewModelEvaluation, NewModelSelection,
        SelectionRecord,
    },
    scope::ScopeSet,
    store::Database,
};
use serde::Deserialize;
use std::{error::Error as StdError, io};

/// The floors a reranker is eligible on; the answer floors belong to the
/// answerer.
const RETRIEVAL_FLOORS: [&str; 3] = ["top_10", "top_1", "search_p95"];

/// A ladder rung's report and the selection it supports.
pub(super) struct RungEvidence<'a> {
    /// The collection whose reranker is selected.
    pub(super) collection: &'a str,
    /// The digest of the reranker's registered card.
    pub(super) card: &'a Digest,
    /// The collection's published generation, the one the rung must have
    /// measured.
    pub(super) published_generation: i64,
    /// The ladder manifest's bytes, stored as the evaluation's manifest.
    pub(super) manifest: &'a [u8],
    /// The rung report's bytes, stored as the evaluation's report.
    pub(super) report: &'a [u8],
    /// Who selects.
    pub(super) selected_by: &'a str,
    /// Why, naming the evidence.
    pub(super) reason: &'a str,
}

/// The parts of a `maestro-eval-ladder-rung/1` or `/2` report a selection checks.
#[derive(Deserialize)]
struct RungReport {
    /// Its contract.
    schema: String,
    /// The rung's name.
    rung: String,
    /// `PASS`, `FAIL` or `INVALID`.
    verdict: String,
    /// The collection it searched.
    collection: String,
    /// What it ran against at its start.
    provenance: Ran,
    /// What it ran against at its end.
    end: Ran,
    /// Its search configuration.
    configuration: Configuration,
    /// Its floors.
    score: Score,
}

/// The parts of a `maestro-ladder-manifest/1` a selection checks.
#[derive(Deserialize)]
struct LadderManifest {
    /// Its contract.
    schema: String,
    /// The collection its rungs search.
    collection: String,
    /// Its rungs.
    rungs: Vec<ManifestRung>,
}

/// A manifest rung's name and reranking settings.
#[derive(Deserialize)]
struct ManifestRung {
    /// Its name.
    name: String,
    /// Its search configuration.
    configuration: Configuration,
}

/// A rung's generation and reranker at one end of its run.
#[derive(Deserialize)]
struct Ran {
    /// The published generation's ID.
    generation: i64,
    /// The reranker card's digest, absent when reranking was off.
    reranker: Option<String>,
}

/// A rung's reranking settings.
#[derive(Deserialize)]
struct Configuration {
    /// The reranker it runs, absent when reranking is off.
    rerank: Option<Rerank>,
}

/// The reranker a rung runs.
#[derive(Deserialize, PartialEq, Eq)]
struct Rerank {
    /// Its card's digest.
    card: String,
    /// The fused candidates it reranks.
    depth: u64,
}

/// A rung's floors.
#[derive(Deserialize)]
struct Score {
    /// Each floor and its status.
    floors: Vec<Floor>,
}

/// One floor's name and status.
#[derive(Deserialize)]
struct Floor {
    /// Its name.
    floor: String,
    /// `pass`, `fail` or `unavailable`.
    status: String,
}

/// Records the rung's report as a real, eligible evaluation of its reranker
/// and selects that reranker, once every check passes and before any write.
pub(super) fn select_reranker(
    database: &Database,
    scopes: &ScopeSet,
    evidence: &RungEvidence<'_>,
) -> Result<SelectionRecord, Box<dyn StdError + Send + Sync>> {
    if evidence.selected_by.trim().is_empty() || evidence.reason.trim().is_empty() {
        return Err(refused("the selection needs who selects and why").into());
    }
    let report: RungReport = serde_json::from_slice(evidence.report)?;
    check_rung(&report, evidence)?;
    let manifest: LadderManifest = serde_json::from_slice(evidence.manifest)?;
    check_manifest(&manifest, &report, evidence.collection)?;
    let card = database
        .model_cards(scopes, evidence.collection, Role::Reranker)?
        .into_iter()
        .find(|record| record.digest == *evidence.card)
        .ok_or_else(|| refused("the card is not a registered reranker of the collection"))?;
    let run_id = format!(
        "ladder:{}:{}",
        report.rung,
        Digest::of(evidence.report).as_str()
    );
    let evaluation = database.record_model_evaluation(
        scopes,
        &NewModelEvaluation {
            run_id: &run_id,
            collection_id: evidence.collection,
            card_id: card.id,
            role: Role::Reranker,
            mode: EvaluationMode::Real,
            generation_id: Some(evidence.published_generation),
            disposition: EvaluationDisposition::Eligible,
            manifest: evidence.manifest,
            report: evidence.report,
        },
    )?;
    Ok(database.record_model_selection(
        scopes,
        &NewModelSelection {
            collection_id: evidence.collection,
            role: Role::Reranker,
            card_id: card.id,
            evaluation_id: evaluation.id,
            selected_by: evidence.selected_by,
            reason: evidence.reason,
        },
    )?)
}

/// Refuses a report that is not a valid measurement of the card on the
/// published generation, or whose retrieval floors do not all pass.
fn check_rung(report: &RungReport, evidence: &RungEvidence<'_>) -> Result<(), io::Error> {
    if !matches!(
        report.schema.as_str(),
        "maestro-eval-ladder-rung/1" | "maestro-eval-ladder-rung/2"
    ) {
        return Err(refused("not a ladder rung report"));
    }
    if report.collection != evidence.collection {
        return Err(refused("the rung searched another collection"));
    }
    if report.verdict == "INVALID" {
        return Err(refused("the rung is INVALID"));
    }
    let card = Some(evidence.card.as_str());
    if [
        report.provenance.reranker.as_deref(),
        report.end.reranker.as_deref(),
        report
            .configuration
            .rerank
            .as_ref()
            .map(|rerank| rerank.card.as_str()),
    ] != [card; 3]
    {
        return Err(refused("the rung ran another reranker"));
    }
    if [report.provenance.generation, report.end.generation] != [evidence.published_generation; 2] {
        return Err(refused("the rung measured another generation"));
    }
    for name in RETRIEVAL_FLOORS {
        if !report
            .score
            .floors
            .iter()
            .any(|floor| floor.floor == name && floor.status == "pass")
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("the rung's {name} floor does not pass"),
            ));
        }
    }
    Ok(())
}

/// Refuses a manifest that did not run the report's rung with its reranker
/// settings on the collection.
fn check_manifest(
    manifest: &LadderManifest,
    report: &RungReport,
    collection: &str,
) -> Result<(), io::Error> {
    if manifest.schema != "maestro-ladder-manifest/1" {
        return Err(refused("not a ladder manifest"));
    }
    if manifest.collection != collection {
        return Err(refused("the manifest searches another collection"));
    }
    if !manifest.rungs.iter().any(|rung| {
        rung.name == report.rung && rung.configuration.rerank == report.configuration.rerank
    }) {
        return Err(refused(
            "the manifest has no rung of the report's name and reranker settings",
        ));
    }
    Ok(())
}

/// A refusal of `message`.
fn refused(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
