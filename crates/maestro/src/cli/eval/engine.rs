//! The ladder's engine on this machine: the kernel opened for the local
//! principal, the model router and the search service. A rung's reranker is
//! the card its manifest names by digest, registered in the collection with
//! the reranker role, and used without being selected; so is a rung's
//! answerer, when its manifest names one.

pub(super) use super::engine_outcome::answer_outcome;

use super::{
    candidates::{candidate_answerer, candidate_reranker},
    documents::{bundle_documents, ranked_documents},
    manifest::{AskSettings, Rung},
    runner::{Asked, Engine, Provenance, RejectedCheck, SearchDiagnostic, Searched},
    stages::{StageFailure, ask_failure, evidence_failure, search_failure, stage_failure},
};
use crate::{
    failure::Failure,
    kernel::{Kernel, pinned_embedder},
    knowledge::{
        operations::{SearchCards, ask::run::registered_answerer, local_search_context},
        source_classes,
    },
};
use maestro_kernel::{
    artifact::Digest,
    gateway::{ModelCard, RouterClient, reply_cap},
    generation::Generation,
};
use maestro_knowledge::{
    answer::{
        Answer, AnswerContext, AnswerPrompt, AskBudget, AskError, AskRequest, DEFAULT_MODEL,
        RegisteredAnswerer, ask_configured,
    },
    eval::{AskOutcome, RunError, SearchOutcome, SectionRef, resolve_expected},
    index::Qdrant,
    search::{
        HydeExpander, IntentExpansion, SearchContext, SearchRequest, SourceClassTable,
        SourceClassifier,
        evidence::{Anchor, ChunkSetDocuments, assemble_evidence},
        search, top_fused_score, top_rerank_score,
    },
    suite::Suite,
};
use std::{error::Error, sync::Arc};
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
    /// The source-class table this machine binds, if any.
    source_classes: Option<Arc<SourceClassTable>>,
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
    /// The explicitly registered expansion card, independent of the answerer.
    intent: Option<ModelCard>,
    /// The rung's reranker, when it reranks.
    reranker: Option<ModelCard>,
    /// The answerer the rung names, or else the one registered for the
    /// default model, if any.
    answerer: Option<RegisteredAnswerer>,
    /// The SHA-256 of the rung's prompt file, when it asks with one.
    prompt: Option<String>,
    /// The digest of the source-class table its source prior reads.
    source_classes: Option<String>,
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
            source_classes: source_classes::load(&kernel.config_dir)?,
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
        let intent = match rung.configuration.intent_expansion {
            IntentExpansion::Off => None,
            IntentExpansion::Hyde => {
                let digest = rung
                    .configuration
                    .intent_card
                    .as_deref()
                    .ok_or_else(|| Failure::refused("hyde requires an explicit intent card"))?;
                let digest = Digest::parse(digest)
                    .map_err(|_| Failure::refused("intent card must be a SHA-256 digest"))?;
                let card = candidate_answerer(kernel, &self.collection, &digest)?.card;
                HydeExpander::new(&self.port, &card)
                    .map_err(|error| Failure::refused_by(&error))?;
                Some(card)
            }
        };
        let answerer = self.answerer(rung)?;
        check_output_limit(rung, answerer.as_ref())?;
        let prompt = rung
            .ask
            .as_ref()
            .and_then(|settings| settings.prompt.digest());
        let source_classes = self
            .source_classes
            .as_ref()
            .filter(|_| rung.configuration.search().source_prior.is_active())
            .map(|table| table.digest().as_str().to_owned());
        Ok(Cards {
            generation,
            embedder,
            intent,
            reranker,
            answerer,
            prompt,
            source_classes,
        })
    }

    /// The answerer `rung` asks with: the card it names, or else the latest
    /// registered answerer of the default model, if any.
    ///
    /// # Errors
    ///
    /// As [`candidate_answerer`], and [`Failure::Failed`] when the default
    /// answerer's card cannot be read.
    pub(super) fn answerer(&self, rung: &Rung) -> Result<Option<RegisteredAnswerer>, Failure> {
        let named = rung
            .ask
            .as_ref()
            .map(AskSettings::answerer_card)
            .transpose()?
            .flatten();
        if let Some(digest) = named {
            return candidate_answerer(self.kernel, &self.collection, &digest).map(Some);
        }
        registered_answerer(
            self.kernel,
            &self.kernel.scopes,
            &self.ask_request("", DEFAULT_MODEL, AskBudget::default()),
        )
        .map_err(|_| Failure::failed("the answerer's card cannot be read"))
    }

    /// The `ask` of `question` in the collection, with `model` and `budget`.
    fn ask_request(&self, question: &str, model: &str, budget: AskBudget) -> AskRequest {
        AskRequest {
            collection: self.collection.clone(),
            question: question.to_owned(),
            model: model.to_owned(),
            version: None,
            budget,
        }
    }

    /// The `ask` of `question` under a rung's `settings` with `answerer`:
    /// its request, with their budget and the answerer's router entry, or
    /// the default model without one, and their prompt; none for a prompt
    /// file not read.
    pub(super) fn ask_call(
        &self,
        question: &str,
        settings: &AskSettings,
        answerer: Option<&RegisteredAnswerer>,
    ) -> Option<(AskRequest, AnswerPrompt)> {
        let model = answerer.map_or(DEFAULT_MODEL, |answerer| {
            answerer.card.fields().router_entry.as_str()
        });
        let prompt = settings.prompt.answer_prompt()?;
        Some((self.ask_request(question, model, settings.budget()), prompt))
    }

    /// The search of `question` in the collection under `rung`'s
    /// configuration, with the budget its asks assemble evidence under: its
    /// ask settings', or the default ask budget when it does not ask.
    pub(super) fn search_request<'a>(
        &'a self,
        rung: &Rung,
        question: &'a str,
    ) -> SearchRequest<'a> {
        let (budget, evidence) = rung.resolved_search_settings();
        SearchRequest {
            configuration: rung.configuration.search(),
            evidence,
            ..SearchRequest::new(&self.collection, question, None, budget)
        }
    }

    /// The search context of the started rung's cards, none before a rung
    /// started.
    pub(super) fn search_context(&self) -> Option<SearchContext<'_, RouterClient>> {
        let cards = &self.held.as_ref()?.cards;
        let mut context = local_search_context(
            &self.kernel.database,
            &self.qdrant,
            &self.port,
            SearchCards {
                embedder: cards.embedder.as_ref(),
                reranker: cards.reranker.as_ref(),
                intent: cards.intent.as_ref(),
            },
        );
        context.source_classes = self
            .source_classes
            .clone()
            .map(|table| table as Arc<dyn SourceClassifier>);
        Some(context)
    }
}

/// How a suite's expected sections that cannot be resolved end the rung: a
/// failed document lookup fails it, and anything else the suite names that
/// the generation does not hold refuses it.
pub(super) fn expected_failure<E: Error + 'static>(error: &RunError<E>) -> Failure {
    match error {
        RunError::Documents { .. } => Failure::failed_by(error),
        _ => Failure::refused_by(error),
    }
}

/// Refuses `rung` when it asks for more output tokens than `answerer`'s card
/// allows: every one of its asks would fail.
fn check_output_limit(rung: &Rung, answerer: Option<&RegisteredAnswerer>) -> Result<(), Failure> {
    let (Some(settings), Some(answerer)) = (&rung.ask, answerer) else {
        return Ok(());
    };
    let limit = answerer.card.fields().limits.output_tokens;
    if limit
        .zip(settings.budget().output_tokens)
        .is_some_and(|(limit, requested)| requested > limit.get())
    {
        return Err(Failure::refused(format!(
            "the rung `{}` asks for more output tokens than its answerer card allows",
            rung.name
        )));
    }
    Ok(())
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
        .map_err(|error| expected_failure(&error))?;
        let provenance = cards.provenance();
        self.held = Some(Held { cards, documents });
        Ok((provenance, expected))
    }

    fn search(&self, rung: &Rung, question: &str) -> Searched {
        let (Some(context), Some(held)) = (self.search_context(), &self.held) else {
            return Searched {
                outcome: SearchOutcome::Failed,
                delivered: Vec::new(),
                diagnostic: SearchDiagnostic::default(),
            };
        };
        let request = self.search_request(rung, question);
        let configuration = request.configuration;
        let mut diagnostic = SearchDiagnostic::default();
        let mut delivered = Vec::new();
        let searched = self.runtime.block_on(async {
            let input = Box::pin(search(&context, &request))
                .await
                .map_err(|error| search_failure(&error))?;
            diagnostic.top_rerank_score = top_rerank_score(&input.ranked);
            diagnostic.top_fused_score = top_fused_score(&input.ranked);
            diagnostic.intent_displaced = input.observations.intent_displaced;
            diagnostic.identifiers_dropped = input.observations.identifiers_dropped.len();
            diagnostic.candidate_source_load_micros =
                input.observations.candidate_source_load_micros;
            diagnostic
                .candidate_context_fallbacks
                .clone_from(&input.observations.candidate_context_fallbacks);
            let order = input.observations.reranked_chunk_ids.clone();
            let database = Arc::clone(&self.kernel.database);
            let bundle = Box::pin(assemble_evidence(
                database,
                input,
                request
                    .evidence
                    .counter()
                    .map_err(|error| evidence_failure(&error))?,
            ))
            .await
            .map_err(|error| evidence_failure(&error))?;
            delivered = bundle.passages.iter().map(Anchor::from).collect();
            diagnostic.evidence_bytes = Some(bundle.budget.evidence_bytes);
            diagnostic.intent_status = bundle.routes.get("intent_expansion").cloned();
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
            delivered,
            diagnostic,
        }
    }

    fn ask(&self, rung: &Rung, question: &str) -> Asked {
        let failed = || Asked {
            outcome: AskOutcome::Failed,
            delivered: Vec::new(),
            rejections: Vec::new(),
            reply_cap: None,
        };
        let (Some(search), Some(held), Some(settings)) =
            (self.search_context(), &self.held, &rung.ask)
        else {
            return failed();
        };
        let Some((request, prompt)) =
            self.ask_call(question, settings, held.cards.answerer.as_ref())
        else {
            return failed();
        };
        let context = AnswerContext {
            search,
            port: &self.port,
            answerer: held.cards.answerer.clone(),
        };
        let configuration = rung.configuration.search();
        let asked = self.runtime.block_on(Box::pin(ask_configured(
            &context,
            &request,
            configuration,
            settings.evidence(),
            &prompt,
        )));
        let cap = asked_reply_cap(
            &asked,
            held.cards.answerer.as_ref(),
            request.budget.output_tokens,
        );
        match asked {
            Ok(answer) => Asked {
                outcome: answer_outcome(
                    &answer,
                    &held.documents,
                    held.cards.generation.id,
                    &configuration,
                ),
                rejections: rejected_checks(&answer),
                reply_cap: cap,
                delivered: answer.delivered,
            },
            Err(error) => Asked {
                outcome: match ask_failure(&error) {
                    StageFailure::TimedOut => AskOutcome::TimedOut,
                    StageFailure::Failed => AskOutcome::Failed,
                },
                delivered: Vec::new(),
                rejections: Vec::new(),
                reply_cap: cap,
            },
        }
    }
}

/// The reply cap the chats of an `ask` that returned `asked` ran with: the
/// answer's; for a chat that timed out or failed, the cap `answerer`'s card
/// gives the `requested` output tokens; none when the ask failed before its
/// chat.
pub(super) fn asked_reply_cap(
    asked: &Result<Answer, AskError>,
    answerer: Option<&RegisteredAnswerer>,
    requested: Option<u32>,
) -> Option<u32> {
    match asked {
        Ok(answer) => answer.reply_cap,
        Err(AskError::TimedOut | AskError::Backend(_)) => {
            answerer.map(|answerer| reply_cap(&answerer.card, requested))
        }
        Err(_) => None,
    }
}

/// The attempts the answer check refused before `answer`, without their
/// tokens.
pub(super) fn rejected_checks(answer: &Answer) -> Vec<RejectedCheck> {
    answer
        .rejections
        .iter()
        .map(|rejection| RejectedCheck {
            attempt: rejection.attempt,
            check: rejection.check,
        })
        .collect()
}

impl Cards {
    /// What a rung with these cards runs against.
    fn provenance(&self) -> Provenance {
        Provenance {
            intent: self.intent.as_ref().map(card_digest),
            generation: self.generation.id,
            chunk_set: self.generation.chunk_set_id.clone(),
            embedder: self.embedder.as_ref().map(card_digest),
            reranker: self.reranker.as_ref().map(card_digest),
            answerer: self
                .answerer
                .as_ref()
                .map(|answerer| card_digest(&answerer.card)),
            prompt: self.prompt.clone(),
            source_classes: self.source_classes.clone(),
        }
    }
}

/// The digest of `card`, in hexadecimal.
fn card_digest(card: &ModelCard) -> String {
    card.digest().as_str().to_owned()
}
