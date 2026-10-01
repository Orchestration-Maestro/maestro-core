//! The comparison across a ladder's rungs, `maestro-eval-ladder-comparison/1`,
//! in JSON and Markdown: each floor per rung and its change from the rung
//! before, with the provenance and ask settings every rung report names. No change is computed
//! to or from an INVALID rung, and a floor a rung did not run shows "not run".

use super::{
    manifest::COMPARISON_NAME,
    reports::{AskReport, Binary, write},
    runner::{Provenance, RungRun, Verdict},
};
use crate::failure::Failure;
use maestro_kernel::artifact::Digest;
use maestro_knowledge::eval::{Floor, FloorStatus, Measure};
use serde::Serialize;
use std::{fmt::Write as _, path::Path};

/// The contract of the comparison across rungs.
const SCHEMA: &str = "maestro-eval-ladder-comparison/1";

/// The comparison across rungs.
#[derive(Debug, Serialize)]
pub(super) struct Comparison<'run> {
    /// Its contract.
    schema: &'static str,
    /// The collection every rung searched.
    collection: &'run str,
    /// The SHA-256 of the suite's file.
    suite_digest: &'run str,
    /// The binary that ran the ladder.
    binary: Binary,
    /// Each rung, in the manifest's order.
    rungs: Vec<ComparedRung<'run>>,
}

/// One rung in the comparison.
#[derive(Debug, Serialize)]
struct ComparedRung<'run> {
    /// Its name.
    rung: &'run str,
    /// Its verdict.
    verdict: Verdict,
    /// What it ran against at its start.
    provenance: &'run Provenance,
    /// What it ran against at its end, absent when it could not be read.
    end: Option<&'run Provenance>,
    /// The settings its asks ran with, absent when it did not ask.
    ask_settings: Option<AskReport>,
    /// Each floor, and its change from the previous rung.
    floors: Vec<ComparedFloor>,
    /// Its supported answers, absent when it did not ask.
    supported_answers: Option<usize>,
    /// Their change from the previous rung.
    supported_change: Option<i64>,
}

/// One floor of a rung in the comparison.
#[derive(Debug, Clone, Copy, Serialize)]
struct ComparedFloor {
    /// The floor.
    floor: Floor,
    /// Whether it holds.
    status: FloorStatus,
    /// What it measured: a count, the invented literals, or a p95 in
    /// microseconds; absent when the p95 did not end or the floor did not run.
    value: Option<i64>,
    /// The value minus the previous rung's, when both have one and neither
    /// rung is INVALID.
    change: Option<i64>,
    /// Whether the rung ran what the floor measures.
    ran: bool,
}

impl ComparedRung<'_> {
    /// What it measured for `floor`, if anything, unless it is INVALID.
    fn value(&self, floor: Floor) -> Option<i64> {
        if self.verdict == Verdict::Invalid {
            return None;
        }
        self.floors
            .iter()
            .find(|compared| compared.floor == floor)
            .and_then(|compared| compared.value)
    }
}

impl<'run> Comparison<'run> {
    /// The comparison of `runs`, over `collection` and the suite of
    /// `suite_digest`, each rung against the one before it.
    pub(super) fn new(
        runs: &'run [RungRun],
        collection: &'run str,
        suite_digest: &'run Digest,
        binary: Binary,
    ) -> Self {
        let mut rungs: Vec<ComparedRung<'run>> = Vec::with_capacity(runs.len());
        for run in runs {
            let verdict = run.verdict();
            let previous = rungs.last().filter(|_| verdict != Verdict::Invalid);
            let floors = run
                .score
                .floors
                .iter()
                .map(|result| {
                    let value = value(result.measure);
                    let before = previous.and_then(|rung| rung.value(result.floor));
                    ComparedFloor {
                        floor: result.floor,
                        status: result.status,
                        value,
                        change: value.zip(before).map(|(now, before)| now - before),
                        ran: !matches!(result.measure, Measure::NotRun { .. }),
                    }
                })
                .collect();
            let supported_answers = run
                .rung
                .ask
                .is_some()
                .then_some(run.score.supported_answers);
            let before = previous
                .filter(|rung| rung.verdict != Verdict::Invalid)
                .and_then(|rung| rung.supported_answers);
            rungs.push(ComparedRung {
                rung: &run.rung.name,
                verdict,
                provenance: &run.start,
                end: run.end.as_ref(),
                ask_settings: run.rung.ask.as_ref().map(AskReport::new),
                floors,
                supported_answers,
                supported_change: supported_answers
                    .zip(before)
                    .map(|(now, before)| count(now) - count(before)),
            });
        }
        Self {
            schema: SCHEMA,
            collection,
            suite_digest: suite_digest.as_str(),
            binary,
            rungs,
        }
    }

    /// The comparison as one Markdown table, a row per rung, then the
    /// provenance of the ladder and of each rung.
    pub(super) fn to_markdown(&self) -> String {
        let mut table = String::from(
            "| Rung | Verdict | Top-10 | Top-1 | Refused | Citation | Answered | Literals \
             | Search p95 | Ask p95 | Supported |\n",
        );
        table.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
        for rung in &self.rungs {
            let _ = write!(table, "| {} | {} |", rung.rung, rung.verdict.name());
            for compared in &rung.floors {
                let _ = write!(table, " {} |", cell(compared));
            }
            let supported = rung
                .supported_answers
                .map_or_else(|| "not run".to_owned(), |answers| answers.to_string());
            let _ = writeln!(
                table,
                " {supported}{} |",
                change(rung.supported_change, false)
            );
        }
        let _ = write!(
            table,
            "\n- Collection: {}\n- Suite digest: {}\n- Binary: {} ({})\n",
            self.collection,
            self.suite_digest,
            self.binary.version,
            self.binary.commit.unwrap_or("commit unknown")
        );
        for rung in &self.rungs {
            let start = rung.provenance;
            let _ = writeln!(
                table,
                "- `{}`: generation {}, chunk set {}, embedder card {}, reranker card {}, \
                 answerer card {}; {}{}",
                rung.rung,
                start.generation,
                start.chunk_set,
                card(start.embedder.as_deref()),
                card(start.reranker.as_deref()),
                card(start.answerer.as_deref()),
                rung.ask_settings.as_ref().map_or_else(
                    || "no ask".to_owned(),
                    |settings| format!("ask: {}", settings.describe())
                ),
                if rung.end == Some(start) {
                    ""
                } else {
                    "; changed while it ran"
                }
            );
        }
        table
    }
}

/// Writes `comparison` under `output`.
///
/// # Errors
///
/// [`Failure::Failed`] when a file cannot be written.
pub(super) fn write_comparison(output: &Path, comparison: &Comparison<'_>) -> Result<(), Failure> {
    let json =
        serde_json::to_string_pretty(comparison).map_err(|error| Failure::failed_by(&error))?;
    write(&output.join(format!("{COMPARISON_NAME}.json")), &json)?;
    write(
        &output.join(format!("{COMPARISON_NAME}.md")),
        &comparison.to_markdown(),
    )
}

/// A card's digest in the table, or "none".
fn card(digest: Option<&str>) -> &str {
    digest.unwrap_or("none")
}

/// What a floor's `measure` gives the comparison.
fn value(measure: Measure) -> Option<i64> {
    match measure {
        Measure::Share { count: met, .. } => Some(count(met)),
        Measure::Literals { invented, .. } => Some(i64::try_from(invented).unwrap_or(i64::MAX)),
        Measure::Latency { p95_us, .. } => {
            p95_us.map(|micros| i64::try_from(micros).unwrap_or(i64::MAX))
        }
        Measure::NotRun { .. } => None,
    }
}

/// `number` as a signed count, saturating.
fn count(number: usize) -> i64 {
    i64::try_from(number).unwrap_or(i64::MAX)
}

/// A floor's cell in the comparison table.
fn cell(compared: &ComparedFloor) -> String {
    if !compared.ran {
        return "not run".to_owned();
    }
    let latency = matches!(compared.floor, Floor::SearchP95 | Floor::AskP95);
    let value = match compared.value {
        Some(micros) if latency => milliseconds(micros),
        Some(number) => number.to_string(),
        None => "not ended".to_owned(),
    };
    format!(
        "{value} {}{}",
        status(compared.status),
        change(compared.change, latency)
    )
}

/// A change from the previous rung, in parentheses, or nothing.
fn change(change: Option<i64>, latency: bool) -> String {
    match change {
        Some(delta) if latency => format!(
            " ({}{})",
            if delta >= 0 { "+" } else { "-" },
            milliseconds(delta.saturating_abs())
        ),
        Some(delta) => format!(" ({delta:+})"),
        None => String::new(),
    }
}

/// `micros` microseconds in milliseconds, to the microsecond.
fn milliseconds(micros: i64) -> String {
    format!("{}.{:03} ms", micros / 1000, (micros % 1000).abs())
}

/// `status` in a table.
const fn status(status: FloorStatus) -> &'static str {
    match status {
        FloorStatus::Pass => "PASS",
        FloorStatus::Fail => "FAIL",
        FloorStatus::Unavailable => "UNAVAILABLE",
    }
}
