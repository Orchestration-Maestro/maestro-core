//! What a ladder writes for each rung: its private rows, one JSON line per
//! question, and its public report in JSON and Markdown. No file holds a
//! question's or an answer's text; the public files hold no row either.

use super::{
    manifest::{AskSettings, RungConfiguration},
    runner::{Provenance, RejectedCheck, RungRun, SearchDiagnostic, Verdict},
};
use crate::failure::Failure;
use maestro_kernel::{artifact::Digest, evidence::RequestBudget};
use maestro_knowledge::search::evidence::EvidenceSettings;
use maestro_knowledge::{
    answer::{AskBudget, RefusalCode},
    eval::{AskOutcome, DeliveryScore, LadderQuestion, LadderScore, SearchOutcome},
    search::evidence::Anchor,
};
use serde::Serialize;
use std::{collections::BTreeMap, fmt::Write as _, fs, path::Path, time::Duration};

/// The contract of a rung's public report.
const RUNG_SCHEMA: &str = "maestro-eval-ladder-rung/2";
/// The directory, under the output directory, of the private rows.
const PRIVATE: &str = "private";

/// The binary that ran the ladder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub(super) struct Binary {
    /// Its package version.
    pub(super) version: &'static str,
    /// The commit it was built from, when the build named one in
    /// `MAESTRO_COMMIT`.
    pub(super) commit: Option<&'static str>,
}

impl Binary {
    /// This binary.
    pub(super) const fn current() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION"),
            commit: option_env!("MAESTRO_COMMIT"),
        }
    }
}

/// A rung's public report, `maestro-eval-ladder-rung/2`.
#[derive(Debug, Serialize)]
pub(super) struct RungReport<'run> {
    /// Its contract.
    schema: &'static str,
    /// The rung's name.
    rung: &'run str,
    /// Its verdict.
    verdict: Verdict,
    /// Whether it asked.
    ask: bool,
    /// The settings its asks ran with, absent when it did not ask.
    ask_settings: Option<AskReport>,
    /// The questions it ran first, unscored.
    warm_ups: usize,
    /// The collection it searched.
    collection: &'run str,
    /// What it ran against at its start.
    provenance: &'run Provenance,
    /// What it ran against at its end; it differs, or is absent when it
    /// could not be read, when the rung is INVALID.
    end: Option<&'run Provenance>,
    /// What the ladder's first rung ran against at its start; the rung is
    /// INVALID when it differs from this in anything but the reranker.
    ladder: &'run Provenance,
    /// Its search configuration.
    configuration: &'run RungConfiguration,
    /// The deadline each of its searches ran with, in milliseconds.
    search_deadline_ms: u32,
    /// The SHA-256 of the suite's file.
    suite_digest: &'run str,
    /// The binary that ran it.
    binary: Binary,
    /// Its floors.
    score: &'run LadderScore,
    /// What the evidence the answerer received delivered: the answerable
    /// questions with an accepted section delivered, delivered whole, the
    /// median coverage, the composition cases and those complete, and the
    /// questions credited with nothing.
    delivery: &'run DeliveryScore,
    /// How many attempts each answer check refused, over every row.
    rejected_checks: BTreeMap<&'static str, usize>,
}

impl<'run> RungReport<'run> {
    /// The report of `run`, over `collection` and the suite of `suite_digest`.
    pub(super) fn new(
        run: &'run RungRun,
        collection: &'run str,
        suite_digest: &'run Digest,
        binary: Binary,
    ) -> Self {
        Self {
            schema: RUNG_SCHEMA,
            rung: &run.rung.name,
            verdict: run.verdict(),
            ask: run.rung.ask.is_some(),
            ask_settings: run.rung.ask.as_ref().map(AskReport::new),
            warm_ups: run.warm_ups,
            collection,
            provenance: &run.start,
            end: run.end.as_ref(),
            ladder: &run.ladder,
            configuration: &run.rung.configuration,
            search_deadline_ms: RequestBudget::default().deadline_ms,
            suite_digest: suite_digest.as_str(),
            binary,
            score: &run.score,
            delivery: &run.delivery,
            rejected_checks: run.rejections.iter().flatten().fold(
                BTreeMap::new(),
                |mut counts, rejection| {
                    *counts.entry(rejection.check).or_default() += 1;
                    counts
                },
            ),
        }
    }

    /// The report as Markdown: its provenance, then its floors.
    pub(super) fn to_markdown(&self) -> String {
        let mut text = format!("# Ladder rung `{}`: {}\n\n", self.rung, self.verdict.name());
        let cards = [
            ("Embedder card", &self.provenance.embedder),
            ("Reranker card", &self.provenance.reranker),
            ("Answerer card", &self.provenance.answerer),
            ("Prompt file", &self.provenance.prompt),
        ];
        let _ = writeln!(text, "- Collection: {}", self.collection);
        let _ = writeln!(text, "- Generation: {}", self.provenance.generation);
        let _ = writeln!(text, "- Chunk set: {}", self.provenance.chunk_set);
        for (name, digest) in cards {
            let _ = writeln!(text, "- {name}: {}", digest.as_deref().unwrap_or("none"));
        }
        let configuration =
            serde_json::to_string(self.configuration).unwrap_or_else(|_| String::new());
        let _ = writeln!(text, "- Configuration: `{configuration}`");
        let _ = writeln!(text, "- Search deadline: {} ms", self.search_deadline_ms);
        let _ = writeln!(text, "- Asks: {}", if self.ask { "yes" } else { "no" });
        let _ = writeln!(
            text,
            concat!(
                "- Citation scoring: same document and pinned revision; exact section ID or ",
                "a cited span intersecting the expected section's extent ",
                "(half-open, max(starts) < min(ends); touching spans do not); ",
                "a whole-document expectation matches any ",
                "citation of that revision."
            )
        );
        if let Some(settings) = &self.ask_settings {
            let _ = writeln!(text, "- Ask settings: {}", settings.describe());
        }
        let _ = writeln!(text, "- Scored bundle: {}", self.scored_bundle());
        text.push_str(&self.delivery.to_markdown());
        let _ = writeln!(
            text,
            "- Rejected answer attempts: {}; the invented-literals floor counts delivered \
             answers only",
            rejected_counts(&self.rejected_checks)
        );
        let _ = writeln!(text, "- Warm-ups: {}", self.warm_ups);
        let _ = writeln!(text, "- Suite digest: {}", self.suite_digest);
        let _ = writeln!(
            text,
            "- Binary: {} ({})",
            self.binary.version,
            self.binary.commit.unwrap_or("commit unknown")
        );
        if self.verdict == Verdict::Invalid {
            text.push_str(
                "- INVALID: the generation or a card changed while the rung ran, or differs \
                 from the first rung's, or could not be read at its end\n",
            );
        }
        text.push('\n');
        text.push_str(&self.score.to_markdown());
        text
    }

    /// The bundle the delivery score reads, in words: what each ask's
    /// answerer received, or each search's evidence under the default ask
    /// budget when the rung did not ask.
    fn scored_bundle(&self) -> String {
        if self.ask {
            return "the evidence each ask gave its answerer, under the ask settings".to_owned();
        }
        let budget = AskBudget::default();
        format!(
            "each search's evidence, assembled under the default ask budget: at most {} \
             passages, {} evidence bytes",
            budget.k, budget.max_tokens
        )
    }
}

/// The settings a rung's asks ran with, each resolved.
#[derive(Debug, Serialize)]
#[expect(
    clippy::min_ident_chars,
    reason = "ask's budget names this limit k, as the manifest does"
)]
pub(super) struct AskReport {
    /// The most passages given to the answerer.
    k: u32,
    /// The evidence budget, in UTF-8 bytes.
    max_tokens: u32,
    /// The most tokens each answerer reply generates.
    output_tokens: u32,
    /// The answer prompt: its version, or `file`.
    prompt: &'static str,
    /// Resolved packing settings.
    evidence: EvidenceSettings,
    /// The deadline of each ask's search, in milliseconds.
    search_deadline_ms: u32,
}

impl AskReport {
    /// The resolved `settings`.
    pub(super) fn new(settings: &AskSettings) -> Self {
        let budget = settings.budget();
        Self {
            k: budget.k,
            max_tokens: budget.max_tokens,
            output_tokens: budget.output_tokens,
            prompt: settings.prompt.name(),
            evidence: settings.evidence(),
            search_deadline_ms: budget.search_deadline_ms,
        }
    }

    /// The settings in words: the most passages, evidence bytes, output
    /// tokens, the prompt and the search deadline.
    pub(super) fn describe(&self) -> String {
        format!(
            "at most {} passages, {} evidence bytes, {} output tokens, prompt {}, search \
             deadline {} ms",
            self.k, self.max_tokens, self.output_tokens, self.prompt, self.search_deadline_ms
        )
    }
}

/// A question's private row: what its search ranked and what its `ask` gave,
/// with no text.
#[derive(Debug, Serialize)]
pub(super) struct PrivateRow<'run> {
    /// The question's ID.
    id: &'run str,
    /// How its search ended: `ranked`, `failed` or `timed_out`.
    search: &'static str,
    /// Its search's time, in microseconds.
    search_us: u64,
    /// The distinct documents of the search's final ranked chunks, before
    /// evidence assembly, the first 10: what the floors score.
    ranked_documents: &'run [String],
    /// The first of them, from 1, that is an expected document.
    expected_rank: Option<usize>,
    /// The anchors of the evidence the answerer received, which the delivery
    /// score reads: the bundle `ask` answered from, or, when the rung does not
    /// ask, the search's evidence under the default ask budget; kept even when
    /// the row is credited with nothing.
    delivered: &'run [Anchor],
    /// The documents of the search's assembled evidence, in rank order: a
    /// diagnostic, assembled under the rung's ask budget.
    bundle_documents: &'run [String],
    /// Source-context loading and validation wall time in microseconds.
    candidate_source_load_micros: u64,
    /// Candidate IDs retained as chunks after whole-unit expansion exceeded its cap.
    candidate_context_fallbacks: &'run [String],
    /// The first of them, from 1, that is an expected document.
    bundle_rank: Option<usize>,
    /// The search's top reranker score, absent when rerank did not run.
    top_rerank_score: Option<f64>,
    /// The search's top fused score, absent when no fused candidate was loaded.
    top_fused_score: Option<f64>,
    /// How its `ask` ended, absent when the rung does not ask.
    ask: Option<&'static str>,
    /// Its `ask`'s time, in microseconds, absent when the rung does not ask.
    ask_us: Option<u64>,
    /// The refusal's code, when `ask` refused.
    refusal: Option<RefusalCode>,
    /// The sections the answer cites.
    citations: Vec<Citation<'run>>,
    /// The attempts the answer check refused before `ask` ended.
    rejections: &'run [RejectedCheck],
}

/// A section an answer cites.
#[derive(Debug, Serialize)]
struct Citation<'run> {
    /// Its document's ID.
    document_id: &'run str,
    /// Its source revision in the rung's pinned chunk set.
    revision_id: Option<&'run str>,
    /// Its chunk's ID.
    chunk_id: Option<&'run str>,
    /// Its ID, absent for a document without sections.
    section_id: Option<&'run str>,
    /// Its half-open byte span in the revision.
    span: Option<[usize; 2]>,
}

impl<'run> PrivateRow<'run> {
    /// The private row of `row`, whose search gave `diagnostic` and whose
    /// `ask` ran when `asked`, after the answer check refused `rejections`.
    pub(super) fn new(
        row: &'run LadderQuestion,
        diagnostic: &'run SearchDiagnostic,
        rejections: &'run [RejectedCheck],
        asked: bool,
    ) -> Self {
        let bundle_documents = diagnostic.bundle_documents.as_slice();
        let (search, ranked_documents): (&'static str, &[String]) = match &row.search.outcome {
            SearchOutcome::Ranked(documents) => ("ranked", documents),
            SearchOutcome::Failed => ("failed", &[]),
            SearchOutcome::TimedOut => ("timed_out", &[]),
        };
        let expected_rank = first_expected(row, ranked_documents);
        let bundle_rank = first_expected(row, bundle_documents);
        let (ask, refusal, citations) = match &row.ask.outcome {
            AskOutcome::Answered { citations, .. } => (
                "answered",
                None,
                citations
                    .iter()
                    .map(|citation| Citation {
                        document_id: &citation.document_id,
                        revision_id: citation.revision_id.as_deref(),
                        chunk_id: citation.chunk_id.as_deref(),
                        section_id: citation.section_id.as_deref(),
                        span: citation.span,
                    })
                    .collect(),
            ),
            AskOutcome::Refused(code) => ("refused", Some(*code), Vec::new()),
            AskOutcome::Failed => ("failed", None, Vec::new()),
            AskOutcome::TimedOut => ("timed_out", None, Vec::new()),
        };
        Self {
            id: &row.id,
            delivered: &row.delivered,
            search,
            search_us: micros(row.search.elapsed),
            ranked_documents,
            expected_rank,
            bundle_documents,
            bundle_rank,
            top_rerank_score: diagnostic.top_rerank_score,
            top_fused_score: diagnostic.top_fused_score,
            candidate_source_load_micros: diagnostic.candidate_source_load_micros,
            candidate_context_fallbacks: &diagnostic.candidate_context_fallbacks,
            ask: asked.then_some(ask),
            ask_us: asked.then(|| micros(row.ask.elapsed)),
            refusal: refusal.filter(|_| asked),
            citations,
            rejections,
        }
    }
}

/// Writes `run`'s private rows and public report under `output`.
///
/// # Errors
///
/// [`Failure::Failed`] when a file cannot be written.
pub(super) fn write_rung(
    output: &Path,
    run: &RungRun,
    report: &RungReport<'_>,
) -> Result<(), Failure> {
    let private = output.join(PRIVATE);
    fs::create_dir_all(&private).map_err(|error| Failure::failed_by(&error))?;
    let mut rows = String::new();
    for ((row, diagnostic), rejections) in
        run.rows.iter().zip(&run.diagnostics).zip(&run.rejections)
    {
        let asked = run.rung.ask.is_some();
        let line = serde_json::to_string(&PrivateRow::new(row, diagnostic, rejections, asked))
            .map_err(|error| Failure::failed_by(&error))?;
        rows.push_str(&line);
        rows.push('\n');
    }
    let name = &run.rung.name;
    write(&private.join(format!("{name}.jsonl")), &rows)?;
    let json = serde_json::to_string_pretty(report).map_err(|error| Failure::failed_by(&error))?;
    write(&output.join(format!("{name}.json")), &json)?;
    write(&output.join(format!("{name}.md")), &report.to_markdown())
}

/// Writes `text` to `path`.
pub(super) fn write(path: &Path, text: &str) -> Result<(), Failure> {
    fs::write(path, text)
        .map_err(|error| Failure::failed(format!("cannot write {}: {error}", path.display())))
}

/// `counts` in words: each check's code and count, or "none".
fn rejected_counts(counts: &BTreeMap<&'static str, usize>) -> String {
    if counts.is_empty() {
        return "none".to_owned();
    }
    counts
        .iter()
        .map(|(check, count)| format!("{check} {count}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The first of `documents`, from 1, that `row` expects.
fn first_expected(row: &LadderQuestion, documents: &[String]) -> Option<usize> {
    documents
        .iter()
        .position(|document| {
            row.expected
                .iter()
                .any(|item| &item.document_id == document)
        })
        .map(|index| index + 1)
}

/// `duration` in whole microseconds, saturating.
fn micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}
