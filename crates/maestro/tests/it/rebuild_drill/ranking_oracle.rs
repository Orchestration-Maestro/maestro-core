//! Exact equality oracle for the synthetic T029c candidate rankings.

use maestro_kernel::{
    chunk_set::Chunk,
    document::{Document, Revision},
    evidence::{RequestBudget, RouteStatus},
    gateway::ModelPort,
    store::Database,
};
use maestro_knowledge::{
    index::Qdrant,
    search::{
        self, DEFAULT_DEPTH, Reranker, SearchContext, SearchRequest, routes::dense::Embedder,
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

pub(super) const SCHEMA: &str = "maestro-synthetic-rankings/1";
pub(super) const REQUIRED_ROUTES: [&str; 3] = ["dense", "lexical", "identifier"];

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct Snapshot {
    pub(super) schema: String,
    pub(super) frozen: Frozen,
    pub(super) generation: i64,
    pub(super) questions: Vec<QuestionRanking>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct Frozen {
    pub(super) suite_digest: String,
    pub(super) collection_digest: String,
    pub(super) corpus_manifest_digest: String,
    pub(super) collection: String,
    pub(super) chunk_set: String,
    pub(super) card_digest: String,
    pub(super) reranker_digest: String,
    pub(super) profiles: BTreeMap<String, String>,
    pub(super) grants: Vec<String>,
    pub(super) backend: BTreeMap<String, String>,
    pub(super) tie_policy: String,
    pub(super) model_identity: String,
    pub(super) qualification_digest: String,
    pub(super) rerank_depth: u32,
    pub(super) point_ids: Vec<String>,
}

pub(super) struct QuestionInput {
    pub(super) id: String,
    pub(super) language: String,
    pub(super) answerable: bool,
    pub(super) question: String,
    pub(super) expected: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct QuestionRanking {
    pub(super) id: String,
    pub(super) language: String,
    pub(super) answerable: bool,
    pub(super) question: String,
    pub(super) expected: Value,
    pub(super) budget: RequestBudget,
    pub(super) routes: BTreeMap<String, RouteStatus>,
    pub(super) inventory: Option<Value>,
    pub(super) known_gaps: Vec<String>,
    pub(super) candidates: Vec<CandidateRanking>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(super) struct CandidateRanking {
    pub(super) chunk_id: String,
    pub(super) document_id: String,
    pub(super) revision_id: String,
    pub(super) source_ref: String,
    pub(super) section_id: Option<String>,
    pub(super) span_start: usize,
    pub(super) span_end: usize,
    pub(super) original_digest: String,
    pub(super) prepared_digest: String,
    pub(super) ranks: BTreeMap<String, u32>,
}

/// The frozen context for capturing each question's production rankings.
pub(super) struct SearchCapture<'a, P> {
    pub(super) database: Arc<Database>,
    pub(super) principal: &'a str,
    pub(super) qdrant: &'a Qdrant,
    pub(super) embedder: &'a Embedder<'a, P>,
    pub(super) reranker: &'a Reranker<'a, P>,
    pub(super) collection: &'a str,
    pub(super) chunks: &'a HashMap<String, Chunk>,
    pub(super) revisions: &'a HashMap<String, Revision>,
    pub(super) documents: &'a HashMap<String, Document>,
}

/// Runs the actual admission, four T029c routes, fusion, and reranker handoff.
pub(super) async fn capture_question<P: ModelPort>(
    capture: &SearchCapture<'_, P>,
    question: &QuestionInput,
) -> Result<QuestionRanking, String> {
    let scopes = capture
        .database
        .visible(capture.principal)
        .map_err(|error| error.to_string())?;
    let request = SearchRequest {
        collection: capture.collection,
        text: &question.question,
        version: None,
        budget: RequestBudget::default(),
        rerank_depth: DEFAULT_DEPTH,
    };
    let context = SearchContext {
        database: Arc::clone(&capture.database),
        principal: capture.principal,
        qdrant: capture.qdrant,
        embedder: Some(Embedder {
            port: capture.embedder.port,
            card: capture.embedder.card,
        }),
        reranker: Some(Reranker {
            port: capture.reranker.port,
            card: capture.reranker.card,
        }),
    };
    let evidence = search::search(&context, &request)
        .await
        .map_err(|error| error.to_string())?;
    if evidence.scopes != Arc::new(scopes) {
        return Err(format!(
            "question {}: admission changed its scope snapshot",
            question.id
        ));
    }
    let candidates = evidence
        .ranked
        .iter()
        .map(|ranked| {
            let fused = &ranked.candidate.fused;
            let chunk = capture.chunks.get(&fused.chunk_id).ok_or_else(|| {
                format!(
                    "question {}: unknown candidate chunk {}",
                    question.id, fused.chunk_id
                )
            })?;
            let revision = capture.revisions.get(&chunk.revision_id).ok_or_else(|| {
                format!(
                    "question {}: unknown revision {}",
                    question.id, chunk.revision_id
                )
            })?;
            let document = capture
                .documents
                .get(&revision.document_id)
                .ok_or_else(|| {
                    format!(
                        "question {}: unknown document {}",
                        question.id, revision.document_id
                    )
                })?;
            Ok(CandidateRanking {
                chunk_id: fused.chunk_id.clone(),
                document_id: revision.document_id.clone(),
                revision_id: revision.id.clone(),
                source_ref: document.source_ref.clone(),
                section_id: chunk.section_id.clone(),
                span_start: chunk.span.start,
                span_end: chunk.span.end,
                original_digest: revision.original_digest.as_str().to_owned(),
                prepared_digest: chunk.digest.as_str().to_owned(),
                ranks: fused
                    .ranks
                    .iter()
                    .map(|(route, rank)| (route.name().to_owned(), rank.get()))
                    .collect(),
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(QuestionRanking {
        id: question.id.clone(),
        language: question.language.clone(),
        answerable: question.answerable,
        question: question.question.clone(),
        expected: question.expected.clone(),
        budget: evidence.budget,
        routes: evidence.routes,
        inventory: evidence
            .inventory
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| error.to_string())?,
        known_gaps: evidence.known_gaps,
        candidates,
    })
}

/// Rejects any changed input, route, question or ordered candidate identity.
pub(super) fn compare_rankings(before: &Snapshot, after: &Snapshot) -> Result<(), String> {
    validate_snapshot(before, "baseline")?;
    validate_snapshot(after, "rebuilt")?;
    if before.schema != SCHEMA || after.schema != SCHEMA {
        return Err("ranking snapshot schema differs".to_owned());
    }
    if before.generation == after.generation {
        return Err(format!(
            "replacement generation {} is not new",
            after.generation
        ));
    }
    if before.frozen != after.frozen {
        return Err(
            "frozen fixture, profile, grant, backend or tie-policy inputs changed".to_owned(),
        );
    }
    if before.questions.len() != after.questions.len() {
        return Err(format!(
            "question count changed: {} != {}",
            before.questions.len(),
            after.questions.len()
        ));
    }
    for (old, new) in before.questions.iter().zip(&after.questions) {
        if old.id != new.id {
            return Err(format!(
                "question order changed at {}: {} != {}",
                old.id, new.id, old.id
            ));
        }
        if old.language != new.language
            || old.answerable != new.answerable
            || old.question != new.question
            || old.expected != new.expected
        {
            return Err(format!(
                "question input or expected labels changed for {}",
                old.id
            ));
        }
        if old.budget != new.budget {
            return Err(format!("request budget changed for {}", old.id));
        }
        if old.routes != new.routes {
            return Err(format!("route availability changed for {}", old.id));
        }
        if old.inventory != new.inventory || old.known_gaps != new.known_gaps {
            return Err(format!(
                "structured inventory or known gaps changed for {}",
                old.id
            ));
        }
        if old.candidates.len() != new.candidates.len() {
            return Err(format!(
                "candidate count changed for {}: {} != {}",
                old.id,
                old.candidates.len(),
                new.candidates.len()
            ));
        }
        for (rank, (old_candidate, new_candidate)) in
            old.candidates.iter().zip(&new.candidates).enumerate()
        {
            if old_candidate != new_candidate {
                return Err(format!(
                    "candidate identity or route rank changed for {} at rank {}",
                    old.id,
                    rank + 1
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_snapshot(snapshot: &Snapshot, label: &str) -> Result<(), String> {
    validate(snapshot, label)
}

fn validate(snapshot: &Snapshot, label: &str) -> Result<(), String> {
    if snapshot.generation <= 0 || snapshot.questions.is_empty() {
        return Err(format!("{label} snapshot has no generation or questions"));
    }
    let mut questions = HashSet::new();
    for question in &snapshot.questions {
        if !questions.insert(&question.id) {
            return Err(format!("{label} has duplicate question {}", question.id));
        }
        if question.answerable && question.candidates.is_empty() {
            return Err(format!(
                "{label} answerable question {} has no candidates",
                question.id
            ));
        }
        for route in REQUIRED_ROUTES {
            if question.routes.get(route) != Some(&RouteStatus::Ok) {
                return Err(format!(
                    "{label} route {route} is degraded for {}",
                    question.id
                ));
            }
        }
        if question.inventory.is_some()
            && question.routes.get("structured") != Some(&RouteStatus::Ok)
        {
            return Err(format!(
                "{label} inventory route is degraded for {}",
                question.id
            ));
        }
        // T029c explicitly degrades Global queries outside its closed inventory grammar.
        let structured_gap = match question.routes.get("structured") {
            Some(RouteStatus::Unavailable(reason))
                if reason.starts_with("unsupported inventory;") =>
            {
                Some(format!("structured route unavailable: {reason}"))
            }
            Some(RouteStatus::Unavailable(_)) => {
                return Err(format!(
                    "{label} structured route failed for {}",
                    question.id
                ));
            }
            _ => None,
        };
        if structured_gap
            .as_ref()
            .is_some_and(|gap| !question.known_gaps.contains(gap))
            || question
                .known_gaps
                .iter()
                .any(|gap| Some(gap) != structured_gap.as_ref())
        {
            return Err(format!(
                "{label} question {} has unexpected known gaps: {:?}",
                question.id, question.known_gaps
            ));
        }
        if !question.candidates.is_empty()
            && question.routes.get("rerank") != Some(&RouteStatus::Ok)
        {
            return Err(format!("{label} reranker is degraded for {}", question.id));
        }
        let mut candidates = HashSet::new();
        for candidate in &question.candidates {
            if !candidates.insert(&candidate.chunk_id) {
                return Err(format!(
                    "{label} question {} repeats chunk {}",
                    question.id, candidate.chunk_id
                ));
            }
        }
    }
    Ok(())
}
