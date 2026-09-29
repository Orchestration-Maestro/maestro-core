//! Converts a checked answer to the ladder's scoring outcome.

use super::stages::{StageFailure, stage_failure};
use maestro_knowledge::{
    answer::Answer,
    eval::{AskOutcome, SectionRef},
    search::{SearchConfiguration, evidence::ChunkSetDocuments},
};

/// What `answer` gives the ladder: its citations' sections and spans, their
/// documents named by `source_ref` in `documents`, or its refusal. A delivered
/// answer passed the answer check, which refuses an invented literal, so it
/// holds none. A generation mismatch drops the citation revisions so scoring
/// cannot accept spans from an unpinned answer.
pub(super) fn answer_outcome(
    answer: &Answer,
    documents: &ChunkSetDocuments,
    pinned_generation: i64,
    configuration: &SearchConfiguration,
) -> AskOutcome {
    match stage_failure(configuration, &answer.routes) {
        Some(StageFailure::TimedOut) => return AskOutcome::TimedOut,
        Some(StageFailure::Failed) => return AskOutcome::Failed,
        None => {}
    }
    if let Some(refusal) = &answer.refusal {
        return AskOutcome::Refused(refusal.code);
    }
    AskOutcome::Answered {
        citations: answer
            .citations
            .iter()
            .map(|citation| SectionRef {
                document_id: documents
                    .document_id(&citation.source_ref)
                    .unwrap_or_default()
                    .to_owned(),
                revision_id: (answer.generation == pinned_generation)
                    .then(|| {
                        documents
                            .revision_id(&citation.source_ref)
                            .map(str::to_owned)
                    })
                    .flatten(),
                chunk_id: Some(citation.chunk_id.clone()),
                section_id: citation.section_id.clone(),
                span: Some(citation.span),
                component: None,
            })
            .collect(),
        invented_literals: 0,
    }
}
