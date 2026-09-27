//! Test-only bridge from the integrated dense/lexical routes and fusion to T021.

use super::{failure::Failure, models::SyntheticModels};
use maestro_canonicalization::CanonicalDocument;
use maestro_kernel::{
    chunk_set::Chunk,
    evidence::{Budget, Bundle, Passage, RequestBudget, RouteStatus, Schema, Trace},
    gateway::ModelCard,
    generation::Generation,
    retrieval::{Error as RetrievalError, ReadControl, SearchRead},
    scope::ScopeSet,
    store::Database,
};
use maestro_knowledge::index::Qdrant;
use maestro_knowledge::{
    search::{
        Fused, Hit, Query, Route, RouteList, fuse,
        routes::{
            dense::{Embedder, search_dense},
            lexical::search_bm25,
        },
    },
    suite::{Language, Question},
};
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    iter,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tokio::{runtime::Runtime, time};

const ROUTE_LIMIT: usize = 80;
const PASSAGE_LIMIT: usize = 10;
const MAX_BYTES: usize = 6000;
const DEADLINE: Duration = Duration::from_millis(10_000);

#[derive(Debug, Clone)]
pub(super) struct SearchCorpus {
    pub(super) revisions: BTreeMap<String, RevisionData>,
    pub(super) documents: BTreeMap<String, CanonicalDocument>,
}

#[derive(Debug, Clone)]
pub(super) struct RevisionData {
    pub(super) canonical: CanonicalDocument,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct QuestionRanking {
    pub(super) question_id: String,
    pub(super) route_status: BTreeMap<String, &'static str>,
    pub(super) ranked: Vec<RankedChunk>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(super) struct RankedChunk {
    pub(super) chunk_id: String,
    pub(super) score: f64,
    pub(super) routes: Vec<String>,
    pub(super) ranks: BTreeMap<String, u32>,
}

/// Resolves only canonical artifacts whose revisions own chunks in `chunk_set`.
/// Exact occurrences map their source references back to that generation member.
pub(super) fn generation_corpus(
    database: &Database,
    scopes: &ScopeSet,
    chunk_set: &str,
) -> Result<SearchCorpus, Failure> {
    let chunks = database
        .chunks(scopes, chunk_set)
        .map_err(|error| Failure::from_error("generation-membership", error))?;
    if chunks.is_empty() {
        return Err(Failure::new("generation-membership"));
    }
    let chunks = chunks
        .into_iter()
        .map(|chunk| (chunk.id.clone(), chunk))
        .collect::<BTreeMap<_, _>>();
    let revision_ids: BTreeSet<String> = chunks
        .values()
        .map(|chunk| chunk.revision_id.clone())
        .collect();
    let mut revisions = BTreeMap::new();
    let mut documents = BTreeMap::new();
    for revision_id in revision_ids {
        let revision = database
            .revision(scopes, &revision_id)
            .map_err(|error| Failure::from_error("generation-membership", error))?
            .ok_or_else(|| Failure::new("generation-membership"))?;
        let document = database
            .document(scopes, &revision.document_id)
            .map_err(|error| Failure::from_error("generation-membership", error))?
            .ok_or_else(|| Failure::new("generation-membership"))?;
        let bytes = database
            .get(&revision.canonical_digest)
            .map_err(|error| Failure::from_error("canonical-artifact", error))?;
        let canonical: CanonicalDocument = serde_json::from_slice(&bytes)
            .map_err(|error| Failure::from_error("canonical-artifact", error))?;
        if canonical.document_id != revision.document_id || canonical.revision_id != revision.id {
            return Err(Failure::new("canonical-artifact-identity"));
        }
        let source_refs = database
            .occurrences(scopes, &revision_id)
            .map_err(|error| Failure::from_error("generation-membership", error))?
            .into_iter()
            .filter(|occurrence| occurrence.collection_id == document.collection_id)
            .map(|occurrence| occurrence.source_ref)
            .chain(iter::once(document.source_ref.clone()));
        for source_ref in source_refs {
            if let Some(previous) = documents.insert(source_ref, canonical.clone())
                && previous.document_id != canonical.document_id
            {
                return Err(Failure::new("ambiguous-duplicate-membership"));
            }
        }
        revisions.insert(revision_id, RevisionData { canonical });
    }
    Ok(SearchCorpus {
        revisions,
        documents,
    })
}

pub(super) struct SearchContext<'a> {
    pub(super) runtime: &'a Runtime,
    pub(super) database: &'a Database,
    pub(super) scopes: &'a ScopeSet,
    pub(super) generation: &'a Generation,
    pub(super) qdrant: &'a Qdrant,
    pub(super) card: &'a ModelCard,
    pub(super) models: &'a SyntheticModels,
    pub(super) corpus: &'a SearchCorpus,
}

struct SearchAttempt<'a> {
    context: &'a SearchContext<'a>,
    question: &'a Question,
    completed: usize,
    started: Instant,
}

/// Searches one suite question using the actual diagnostic routes and RRF,
/// then resolves the selected chunks from the pinned generation's authority.
pub(super) fn retrieve(
    context: &SearchContext<'_>,
    question: &Question,
    completed: usize,
) -> Result<(Bundle, QuestionRanking), Failure> {
    let attempt = SearchAttempt {
        context,
        question,
        completed,
        started: Instant::now(),
    };
    let deadline = time::Instant::from_std(attempt.started + DEADLINE);
    let query = Query {
        generation: context.generation,
        scopes: context.scopes,
        text: &question.question,
        limit: ROUTE_LIMIT,
        version: None,
        qdrant: context.qdrant,
    };
    let embedder = Embedder {
        port: context.models,
        card: context.card,
    };
    let routes = context.runtime.block_on(async {
        time::timeout_at(deadline, async {
            let dense = search_dense(&query, &embedder).await.map_err(|error| {
                Failure::question_from_error("dense", &question.id, completed, error)
            })?;
            let lexical = search_bm25(&query).await.map_err(|error| {
                Failure::question_from_error("lexical", &question.id, completed, error)
            })?;
            Ok::<_, Failure>((dense, lexical))
        })
        .await
    });
    let (dense, lexical) = match routes {
        Ok(Ok(routes)) => routes,
        Ok(Err(failure)) => return Err(failure),
        Err(error) => {
            return Err(Failure::question_from_error(
                "deadline",
                &question.id,
                completed,
                error,
            ));
        }
    };
    let lists = [
        RouteList {
            route: Route::Dense,
            hits: dense
                .into_iter()
                .map(|hit| Hit {
                    chunk_id: hit.chunk_id,
                    score: hit.score,
                })
                .collect(),
        },
        RouteList {
            route: Route::Lexical,
            hits: lexical
                .into_iter()
                .map(|hit| Hit {
                    chunk_id: hit.chunk_id,
                    score: hit.score,
                })
                .collect(),
        },
    ];
    let ranked = fuse(&lists, ROUTE_LIMIT);
    if ranked.iter().any(|hit| !hit.score.is_finite()) {
        return Err(Failure::question("fusion", &question.id, completed));
    }
    let chunks = authorized_chunks(&attempt, &ranked)?;
    let snapshot = QuestionRanking {
        question_id: question.id.clone(),
        route_status: BTreeMap::from([
            (Route::Dense.name().to_owned(), "ok"),
            (Route::Lexical.name().to_owned(), "ok"),
        ]),
        ranked: ranked.iter().map(ranked_chunk).collect(),
    };
    let bundle = evidence_bundle(&attempt, &ranked, &chunks)?;
    Ok((bundle, snapshot))
}

/// Reads only route-returned chunks through T029c's scoped, deadline-controlled reader.
fn authorized_chunks(
    attempt: &SearchAttempt<'_>,
    ranked: &[Fused],
) -> Result<BTreeMap<String, Chunk>, Failure> {
    let ids: Vec<String> = ranked.iter().map(|hit| hit.chunk_id.clone()).collect();
    let control = ReadControl {
        deadline: attempt.started + DEADLINE,
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let read = SearchRead {
        generation: attempt.context.generation,
        scopes: attempt.context.scopes,
        version: None,
        control: &control,
    };
    let chunks = attempt
        .context
        .database
        .search_chunks(&read, &ids)
        .map_err(|error| {
            let stage = match error {
                RetrievalError::TimedOut => "deadline",
                RetrievalError::Cancelled => "cancelled",
                _ => "generation-membership",
            };
            Failure::question_from_error(stage, &attempt.question.id, attempt.completed, error)
        })?;
    let chunks: BTreeMap<String, Chunk> = chunks
        .into_iter()
        .map(|chunk| (chunk.id.clone(), chunk))
        .collect();
    if chunks.len() != ids.len() || ids.iter().any(|id| !chunks.contains_key(id)) {
        return Err(Failure::question(
            "generation-membership",
            &attempt.question.id,
            attempt.completed,
        ));
    }
    Ok(chunks)
}

/// The actual RRF identity and per-route positions, before passage selection.
fn ranked_chunk(hit: &Fused) -> RankedChunk {
    RankedChunk {
        chunk_id: hit.chunk_id.clone(),
        score: hit.score,
        routes: hit
            .ranks
            .keys()
            .map(|route| route.name().to_owned())
            .collect(),
        ranks: hit
            .ranks
            .iter()
            .map(|(route, rank)| (route.name().to_owned(), rank.get()))
            .collect(),
    }
}

/// A minimal T021 bundle adapter; T032 owns the final evidence assembly.
fn evidence_bundle(
    attempt: &SearchAttempt<'_>,
    ranked: &[Fused],
    chunks: &BTreeMap<String, Chunk>,
) -> Result<Bundle, Failure> {
    let context = attempt.context;
    let database = context.database;
    let scopes = context.scopes;
    let generation = context.generation;
    let corpus = context.corpus;
    let question = attempt.question;
    let completed = attempt.completed;
    let started = attempt.started;
    let mut passages = Vec::new();
    let mut trace = Vec::new();
    let mut evidence_bytes = 0_usize;
    for (index, hit) in ranked.iter().take(PASSAGE_LIMIT).enumerate() {
        let chunk = chunks
            .get(&hit.chunk_id)
            .ok_or_else(|| Failure::question("chunk-membership", &question.id, completed))?;
        let excerpt = database
            .resolve(scopes, &generation.chunk_set_id, &chunk.id)
            .map_err(|error| {
                Failure::question_from_error("source-resolution", &question.id, completed, error)
            })?;
        let revision = corpus
            .revisions
            .get(&chunk.revision_id)
            .ok_or_else(|| Failure::question("canonical-artifact", &question.id, completed))?;
        let section_path = match &chunk.section_id {
            Some(section_id) => revision
                .canonical
                .sections
                .iter()
                .find(|section| section.section_id == *section_id)
                .map(|section| section.heading_path.clone())
                .ok_or_else(|| Failure::question("section-membership", &question.id, completed))?,
            None => Vec::new(),
        };
        let text_bytes = excerpt.text.len();
        if evidence_bytes.saturating_add(text_bytes) > MAX_BYTES {
            break;
        }
        evidence_bytes += text_bytes;
        let number = u32::try_from(passages.len() + 1).map_err(|error| {
            Failure::question_from_error("passage-accounting", &question.id, index, error)
        })?;
        let routes: Vec<String> = hit
            .ranks
            .keys()
            .map(|route| route.name().to_owned())
            .collect();
        passages.push(Passage {
            n: number,
            section_id: excerpt.section_id,
            document_id: excerpt.document_id,
            revision_id: excerpt.revision_id,
            title: revision
                .canonical
                .source_metadata
                .title
                .clone()
                .unwrap_or_default(),
            section_path,
            version: excerpt.version,
            source_ref: excerpt.source_ref,
            span: excerpt.span,
            digest: excerpt.digest,
            text: excerpt.text,
            windowed: false,
            alternates: Vec::new(),
        });
        trace.push(Trace {
            n: number,
            score: Some(hit.score),
            routes,
            chunk_ids: vec![chunk.id.clone()],
            procedural: false,
        });
    }
    if Instant::now().duration_since(started) > DEADLINE {
        return Err(Failure::question("deadline", &question.id, completed));
    }
    test_bundle(attempt, passages, trace, evidence_bytes)
}

fn test_bundle(
    attempt: &SearchAttempt<'_>,
    passages: Vec<Passage>,
    trace: Vec<Trace>,
    evidence_bytes: usize,
) -> Result<Bundle, Failure> {
    let question = attempt.question;
    let generation = attempt.context.generation;
    let language = match question.language {
        Language::Fr => "fr",
        Language::En => "en",
    };
    let budget = u32::try_from(evidence_bytes).map_err(|error| {
        Failure::question_from_error("evidence-budget", &question.id, attempt.completed, error)
    })?;
    Ok(Bundle {
        schema: Schema::V1,
        collection: generation.collection_id.clone(),
        generation: generation.id,
        query: question.question.clone(),
        lang: language.to_owned(),
        routes: BTreeMap::from([
            (Route::Dense.name().to_owned(), RouteStatus::Ok),
            (Route::Lexical.name().to_owned(), RouteStatus::Ok),
        ]),
        passages,
        conflicts: Vec::new(),
        known_gaps: Vec::new(),
        budget: Budget {
            evidence_tokens: budget,
            limit: u32::try_from(MAX_BYTES).unwrap_or(u32::MAX),
            counter: None,
            estimated: false,
        },
        request_budget: Some(RequestBudget {
            k: u32::try_from(PASSAGE_LIMIT).unwrap_or(u32::MAX),
            max_tokens: u32::try_from(MAX_BYTES).unwrap_or(u32::MAX),
            deadline_ms: u32::try_from(DEADLINE.as_millis()).unwrap_or(u32::MAX),
        }),
        inventory: None,
        trace,
    })
}
