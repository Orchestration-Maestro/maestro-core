//! The ladder's engine on this machine: the kernel opened for the local
//! principal, the model router and the search service. A rung's reranker is
//! the card its manifest names by digest, registered in the collection with
//! the reranker role, and used without being selected.

use super::{
    manifest::Rung,
    runner::{Engine, Provenance, SearchDiagnostic, Searched},
    stages::{StageFailure, stage_failure},
};
use crate::{
    failure::Failure,
    kernel::{Kernel, pinned_embedder},
    knowledge::operations::ask::run::registered_answerer,
};
use maestro_kernel::{
    artifact::Digest,
    evidence::{Bundle, RequestBudget},
    gateway::{ModelCard, Role, RouterClient},
    generation::Generation,
    scope::LOCAL,
};
use maestro_knowledge::{
    answer::{
        Answer, AnswerContext, AskBudget, AskError, AskRequest, DEFAULT_MODEL, RegisteredAnswerer,
        ask_configured,
    },
    eval::{AskOutcome, RunError, SearchOutcome, SectionRef, resolve_expected},
    index::Qdrant,
    search::{
        Reranker, SearchConfiguration, SearchContext, SearchError, SearchRequest,
        evidence::{ChunkSetDocuments, EvidenceCounter, EvidenceError, assemble_evidence},
        routes::dense::Embedder,
        search, top_fused_score, top_rerank_score,
    },
    suite::Suite,
};
use std::{collections::BTreeMap, sync::Arc};
use tokio::runtime::{Builder, Runtime};

/// The engine of the kernel, the router and the search service.
pub(super) struct KernelEngine<'kernel> {
    /// The kernel, opened for the local principal.
    kernel: &'kernel Kernel,
    /// The collection every rung searches.
    collection: String,
    /// The model router.
    port: RouterClient,
    /// The search service.
    qdrant: Qdrant,
    /// The runtime each search and ask runs on, one at a time.
    runtime: Runtime,
    /// What the current rung runs against, once it started.
    held: Option<Held>,
}

/// What a started rung runs against.
struct Held {
    /// Its cards and generation.
    cards: Cards,
    /// The documents of the generation's chunk set.
    documents: ChunkSetDocuments,
}

/// A rung's generation and cards.
struct Cards {
    /// The collection's published generation.
    generation: Generation,
    /// The embedder its dense vectors were made with, if its card loads.
    embedder: Option<ModelCard>,
    /// The rung's reranker, when it reranks.
    reranker: Option<ModelCard>,
    /// The answerer registered for the default model, if any.
    answerer: Option<RegisteredAnswerer>,
}

impl<'kernel> KernelEngine<'kernel> {
    /// The engine of `kernel`, `port` and `qdrant` for `collection`.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] when the runtime cannot start.
    pub(super) fn new(
        kernel: &'kernel Kernel,
        collection: &str,
        port: RouterClient,
        qdrant: Qdrant,
    ) -> Result<Self, Failure> {
        let runtime = Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| Failure::failed_by(&error))?;
        Ok(Self {
            kernel,
            collection: collection.to_owned(),
            port,
            qdrant,
            runtime,
            held: None,
        })
    }

    /// The generation and cards `rung` runs against now.
    fn cards(&self, rung: &Rung) -> Result<Cards, Failure> {
        let kernel = self.kernel;
        let generation = kernel
            .database
            .published_generation(&kernel.scopes, &self.collection)
            .map_err(|error| Failure::failed_by(&error))?
            .ok_or_else(|| Failure::refused("the collection has no published generation"))?;
        let embedder = pinned_embedder(&kernel.artifacts, Some(&generation.embedding_profile));
        let reranker =
            candidate_reranker(kernel, &self.collection, rung.configuration.reranker()?)?;
        let answerer = registered_answerer(kernel, &kernel.scopes, &self.ask_request(""))
            .map_err(|_| Failure::failed("the answerer's card cannot be read"))?;
        Ok(Cards {
            generation,
            embedder,
            reranker,
            answerer,
        })
    }

    /// The `ask` of `question` in the collection, with the default model and
    /// budget.
    fn ask_request(&self, question: &str) -> AskRequest {
        AskRequest {
            collection: self.collection.clone(),
            question: question.to_owned(),
            model: DEFAULT_MODEL.to_owned(),
            version: None,
            budget: AskBudget::default(),
        }
    }

    /// The search context of the started rung's cards, none before a rung
    /// started.
    pub(super) fn search_context(&self) -> Option<SearchContext<'_, RouterClient>> {
        let cards = &self.held.as_ref()?.cards;
        Some(SearchContext {
            database: Arc::clone(&self.kernel.database),
            principal: LOCAL,
            qdrant: &self.qdrant,
            embedder: cards.embedder.as_ref().map(|card| Embedder {
                port: &self.port,
                card,
            }),
            reranker: cards.reranker.as_ref().map(|card| Reranker {
                port: &self.port,
                card,
            }),
        })
    }
}

impl Engine for KernelEngine<'_> {
    fn provenance(&self, rung: &Rung) -> Result<Provenance, Failure> {
        Ok(self.cards(rung)?.provenance())
    }

    fn start(
        &mut self,
        rung: &Rung,
        suite: &Suite,
    ) -> Result<(Provenance, Vec<Vec<SectionRef>>), Failure> {
        self.held = None;
        let cards = self.cards(rung)?;
        let database = &self.kernel.database;
        let documents = ChunkSetDocuments::read(
            database,
            &self.kernel.scopes,
            &cards.generation.chunk_set_id,
        )
        .map_err(|error| Failure::failed_by(&error))?;
        let expected = resolve_expected(suite, |source_ref| {
            documents.canonical(database, source_ref)
        })
        .map_err(|error| match error {
            RunError::Documents { .. } => Failure::failed_by(&error),
            _ => Failure::refused_by(&error),
        })?;
        let provenance = cards.provenance();
        self.held = Some(Held { cards, documents });
        Ok((provenance, expected))
    }

    fn search(&self, rung: &Rung, question: &str) -> Searched {
        let (Some(context), Some(held)) = (self.search_context(), &self.held) else {
            return Searched {
                outcome: SearchOutcome::Failed,
                diagnostic: SearchDiagnostic::default(),
            };
        };
        let configuration = rung.configuration.search();
        let request = SearchRequest {
            configuration,
            ..SearchRequest::new(&self.collection, question, None, RequestBudget::default())
        };
        let mut diagnostic = SearchDiagnostic::default();
        let searched = self.runtime.block_on(async {
            let input = Box::pin(search(&context, &request))
                .await
                .map_err(|error| search_failure(&error))?;
            diagnostic.top_rerank_score = top_rerank_score(&input.ranked);
            diagnostic.top_fused_score = top_fused_score(&input.ranked);
            let order = input.observations.reranked_chunk_ids.clone();
            let database = Arc::clone(&self.kernel.database);
            let bundle = Box::pin(assemble_evidence(
                database,
                input,
                EvidenceCounter::Utf8Bytes,
            ))
            .await
            .map_err(|error| evidence_failure(&error))?;
            diagnostic.bundle_documents = bundle_documents(&bundle, &order);
            match stage_failure(&configuration, &bundle.routes) {
                Some(failure) => Err(failure),
                None => Ok(ranked_documents(&order, |chunk| {
                    held.documents.document_of_chunk(chunk)
                })),
            }
        });
        Searched {
            outcome: match searched {
                Ok(ranked) => SearchOutcome::Ranked(ranked),
                Err(StageFailure::TimedOut) => SearchOutcome::TimedOut,
                Err(StageFailure::Failed) => SearchOutcome::Failed,
            },
            diagnostic,
        }
    }

    fn ask(&self, rung: &Rung, question: &str) -> AskOutcome {
        let (Some(search), Some(held)) = (self.search_context(), &self.held) else {
            return AskOutcome::Failed;
        };
        let context = AnswerContext {
            search,
            port: &self.port,
            answerer: held.cards.answerer.clone(),
        };
        let request = self.ask_request(question);
        let configuration = rung.configuration.search();
        let asked =
            self.runtime
                .block_on(Box::pin(ask_configured(&context, &request, configuration)));
        match asked {
            Ok(answer) => answer_outcome(
                &answer,
                &held.documents,
                held.cards.generation.id,
                &configuration,
            ),
            Err(error) => match ask_failure(&error) {
                StageFailure::TimedOut => AskOutcome::TimedOut,
                StageFailure::Failed => AskOutcome::Failed,
            },
        }
    }
}

impl Cards {
    /// What a rung with these cards runs against.
    fn provenance(&self) -> Provenance {
        Provenance {
            generation: self.generation.id,
            chunk_set: self.generation.chunk_set_id.clone(),
            embedder: self.embedder.as_ref().map(card_digest),
            reranker: self.reranker.as_ref().map(card_digest),
            answerer: self
                .answerer
                .as_ref()
                .map(|answerer| card_digest(&answerer.card)),
        }
    }
}

/// The digest of `card`, in hexadecimal.
fn card_digest(card: &ModelCard) -> String {
    card.digest().as_str().to_owned()
}

/// The reranker card of `digest`, registered in `collection`, without
/// selecting it; none when `digest` is none.
///
/// # Errors
///
/// [`Failure::Refused`] for a card the collection has not registered, or
/// whose role is not reranker; [`Failure::Failed`] when the kernel cannot be
/// read.
pub(super) fn candidate_reranker(
    kernel: &Kernel,
    collection: &str,
    digest: Option<Digest>,
) -> Result<Option<ModelCard>, Failure> {
    let Some(digest) = digest else {
        return Ok(None);
    };
    let card = kernel
        .database
        .model_card(&kernel.scopes, collection, &digest)
        .map_err(|error| Failure::failed_by(&error))?
        .ok_or_else(|| {
            Failure::refused("a rung's reranker card is not registered in the collection")
        })?;
    if card.fields().role != Role::Reranker {
        return Err(Failure::refused(
            "a rung's reranker card does not have the reranker role",
        ));
    }
    Ok(Some(card))
}

/// The most documents a search's ranked list holds: the floors score its
/// first 10.
const RANKED_DOCUMENTS: usize = 10;

/// The distinct documents of `order`, the chunks after reranking, or in fused
/// order when no rerank ran, before evidence assembly: each at its best
/// chunk's rank, the first [`RANKED_DOCUMENTS`]. `document_of` names each
/// chunk's document; a chunk it does not know is skipped.
pub(super) fn ranked_documents<'set>(
    order: &[String],
    document_of: impl Fn(&str) -> Option<&'set str>,
) -> Vec<String> {
    let mut ranked: Vec<String> = Vec::with_capacity(RANKED_DOCUMENTS);
    for document in order.iter().filter_map(|chunk| document_of(chunk)) {
        if ranked.len() == RANKED_DOCUMENTS {
            break;
        }
        if !ranked.iter().any(|seen| seen == document) {
            ranked.push(document.to_owned());
        }
    }
    ranked
}

/// The documents of `bundle`'s passages in retrieval rank, as `order`, the
/// chunks after reranking, ranks them: each passage at the best rank of its
/// chunks, a passage without a ranked chunk last, ties by passage number.
pub(super) fn bundle_documents(bundle: &Bundle, order: &[String]) -> Vec<String> {
    let rank: BTreeMap<&str, usize> = order
        .iter()
        .enumerate()
        .rev()
        .map(|(index, chunk)| (chunk.as_str(), index))
        .collect();
    let mut passages: Vec<(usize, u32, &str)> = bundle
        .passages
        .iter()
        .map(|passage| {
            let best = bundle
                .trace
                .iter()
                .filter(|trace| trace.n == passage.n)
                .flat_map(|trace| &trace.chunk_ids)
                .filter_map(|chunk| rank.get(chunk.as_str()).copied())
                .min()
                .unwrap_or(usize::MAX);
            (best, passage.n, passage.document_id.as_str())
        })
        .collect();
    passages.sort_unstable();
    passages
        .into_iter()
        .map(|(_, _, document)| document.to_owned())
        .collect()
}

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
            })
            .collect(),
        invented_literals: 0,
    }
}

/// How a search that returned `error` ended: out of time when its admission
/// or a permission check ran out of time, failed otherwise.
pub(super) const fn search_failure(error: &SearchError) -> StageFailure {
    match error {
        SearchError::AdmissionTimedOut | SearchError::PermissionCheckTimedOut => {
            StageFailure::TimedOut
        }
        SearchError::InvalidRequest { .. }
        | SearchError::Admission(_)
        | SearchError::Kernel(_)
        | SearchError::EvidenceLoad { .. }
        | SearchError::PermissionsChanged
        | SearchError::WorkerFailed => StageFailure::Failed,
    }
}

/// How evidence assembly that returned `error` ended.
pub(super) const fn evidence_failure(error: &EvidenceError) -> StageFailure {
    if matches!(error, EvidenceError::TimedOut) {
        StageFailure::TimedOut
    } else {
        StageFailure::Failed
    }
}

/// How an `ask` that returned `error` ended: out of time when its search,
/// its evidence or its answerer ran out of time, failed otherwise.
pub(super) const fn ask_failure(error: &AskError) -> StageFailure {
    match error {
        AskError::TimedOut => StageFailure::TimedOut,
        AskError::Search(error) => search_failure(error),
        AskError::Evidence(error) => evidence_failure(error),
        AskError::InvalidRequest(_)
        | AskError::Backend(_)
        | AskError::EvidenceIntegrity
        | AskError::Json(_) => StageFailure::Failed,
    }
}
