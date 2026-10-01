//! Run identity and result types shared by the synthetic pipeline stages.

use super::super::search::{QuestionRanking, RankedChunk};
use maestro_knowledge::eval::Report;
use serde::Serialize;
use std::collections::BTreeMap;

pub(super) const COLLECTION: &str = "synthetic";
pub(super) const SEED: u64 = 20_260_927;
pub(super) const DECLARATION_DIGEST: &str =
    "408eb4619c107273d9a5d4188bfa0036391994695d6c5d020eaab7b4fcf2498a";
pub(super) const MANIFEST_DIGEST: &str =
    "38eddbbd15096a58f0a1d17ff790689bf2e556e46113b07fff22bdc78fa58e0a";
pub(super) const SUITE_DIGEST: &str =
    "ff333fb8214dcf2499f1ddcc3d686908bdfd738e92d49f32647d229654d6aee9";
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::synthetic_gate) struct Counts {
    pub(in crate::synthetic_gate) imported: u64,
    pub(in crate::synthetic_gate) quality_decided: u64,
    pub(in crate::synthetic_gate) accepted: u64,
    pub(in crate::synthetic_gate) eligible: u64,
    pub(in crate::synthetic_gate) prepared: u64,
    pub(in crate::synthetic_gate) duplicates: u64,
    pub(in crate::synthetic_gate) chunks: u64,
    pub(in crate::synthetic_gate) published_points: u64,
    pub(in crate::synthetic_gate) dense_queries: u64,
    pub(in crate::synthetic_gate) completed_questions: u64,
}

#[derive(Debug)]
pub(in crate::synthetic_gate) struct Output {
    pub(in crate::synthetic_gate) report: Report,
    pub(in crate::synthetic_gate) rankings: Vec<QuestionRanking>,
    pub(in crate::synthetic_gate) counts: Counts,
    pub(in crate::synthetic_gate) provenance: Provenance,
    pub(in crate::synthetic_gate) baseline_digest: Option<String>,
    pub(in crate::synthetic_gate) model_identity: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(in crate::synthetic_gate) struct Provenance {
    pub(super) schema: &'static str,
    pub(super) source_commit: Option<String>,
    pub(super) suite: &'static str,
    pub(super) suite_digest: String,
    pub(super) declaration_digest: String,
    pub(super) manifest_digest: String,
    pub(super) corpus_entries: Vec<CorpusDigest>,
    pub(super) backend: String,
    pub(super) model: &'static str,
    pub(super) model_card_digest: String,
    pub(super) tokenizer_qualification: String,
    pub(super) profiles: BTreeMap<String, String>,
    pub(super) answerable_questions: usize,
    pub(super) unanswerable_questions: usize,
    pub(super) baseline_digest: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CorpusDigest {
    pub(super) path: String,
    pub(super) digest: String,
    pub(super) bytes: u64,
}

pub(super) fn same_run_identity(first: &Output, second: &Output) -> bool {
    if first.rankings != second.rankings {
        let changed = first
            .rankings
            .iter()
            .zip(&second.rankings)
            .filter(|(left, right)| left != right)
            .collect::<Vec<_>>();
        eprintln!(
            "repeated search rankings differ for {} questions: {:?}",
            changed.len(),
            changed
                .iter()
                .map(|(left, _)| &left.question_id)
                .collect::<Vec<_>>(),
        );
        for (left, right) in changed {
            log_rank_movements(&left.question_id, &left.ranked, &right.ranked);
        }
        return false;
    }
    let mut first_report = first.report.clone();
    let mut second_report = second.report.clone();
    for report in [&mut first_report, &mut second_report] {
        for question in &mut report.questions {
            question.latency_us = 0;
        }
        report.metrics.latency_p50_us = None;
        report.metrics.latency_p95_us = None;
    }
    if first_report != second_report {
        if first_report.generation != second_report.generation {
            eprintln!(
                "repeated report generation differs: {} vs {}",
                first_report.generation, second_report.generation
            );
        }
        if first_report.profiles != second_report.profiles {
            eprintln!("repeated report profile identities differ");
        }
        if let Some((left, right)) = first_report
            .questions
            .iter()
            .zip(&second_report.questions)
            .find(|(left, right)| left != right)
        {
            eprintln!(
                "first repeated question result difference at `{}`: {:?} vs {:?}",
                left.id, left, right
            );
        }
        if first_report.metrics != second_report.metrics {
            eprintln!(
                "repeated non-latency metric values differ: {:?} vs {:?}",
                first_report.metrics, second_report.metrics
            );
        }
        return false;
    }
    true
}

pub(in crate::synthetic_gate) fn assert_repeated_run_identity(first: &Output, second: &Output) {
    assert!(same_run_identity(first, second));
}

fn log_rank_movements(question_id: &str, first: &[RankedChunk], second: &[RankedChunk]) {
    let positions = |ranked: &[RankedChunk], route: &str| {
        ranked
            .iter()
            .filter_map(|hit| {
                hit.ranks
                    .get(route)
                    .map(|rank| (hit.chunk_id.clone(), *rank))
            })
            .collect::<BTreeMap<_, _>>()
    };
    for route in ["dense", "lexical"] {
        let before = positions(first, route);
        let after = positions(second, route);
        let changed = before
            .iter()
            .find(|(chunk_id, rank)| after.get(*chunk_id) != Some(*rank));
        if let Some((chunk_id, before_rank)) = changed {
            eprintln!(
                "rank movement for `{question_id}` route `{route}`: chunk `{chunk_id}` \
                 position {before_rank} -> {:?}",
                after.get(chunk_id),
            );
        }
    }
}
