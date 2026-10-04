//! The bounded search-to-answer state machine and refusal handling.

use super::{
    prompt::{chat_request, prompt},
    types::{
        ANSWER_SCHEMA, Answer, AnswerCitation, AnswerContext, AnswerModel, AnswerPrompt,
        AnswerRefusal, AskError, AskRequest, CHAT_DEADLINE, CLOSEST_LIMIT, LanguageCheck,
        PromptVersion, RefusalCode, RegisteredAnswerer, Rejection, ResponseLanguage,
    },
    validate::{Invalid, Reply, ValidReply, ValidationFailure, validate_reply},
};
use crate::{
    query::{Language, understand},
    search::{
        SearchConfiguration, SearchRequest,
        evidence::{Anchor, EvidenceSettings, assemble_evidence},
        search, top_rerank_score,
    },
};
use maestro_kernel::{
    evidence::{Bundle, RequestBudget},
    gateway::{
        Error as GatewayError, Message, ModelPort, Role, Room, RouterEntry, Speaker, reply_cap,
    },
};
use std::collections::BTreeMap;
use tokio::time::timeout;

/// Searches once with the default configuration, assembles verified passages
/// and validates at most two chat replies.
///
/// # Errors
/// Returns an error for invalid bounds, failed search or evidence integrity,
/// unavailable chat, or an expired chat deadline.
pub async fn ask<P: ModelPort + Sync>(
    context: &AnswerContext<'_, P>,
    request: &AskRequest,
) -> Result<Answer, AskError> {
    ask_configured(
        context,
        request,
        SearchConfiguration::default(),
        EvidenceSettings::default(),
        &PromptVersion::default().into(),
    )
    .await
}

/// As [`ask`], with its search run under `configuration` and its answerer
/// given `answer_prompt`.
///
/// # Errors
/// As [`ask`].
pub async fn ask_configured<P: ModelPort + Sync>(
    context: &AnswerContext<'_, P>,
    request: &AskRequest,
    configuration: SearchConfiguration,
    evidence: EvidenceSettings,
    answer_prompt: &AnswerPrompt,
) -> Result<Answer, AskError> {
    validate_request(request)?;
    let counter = evidence.counter().map_err(AskError::Evidence)?;
    let search_request = SearchRequest {
        configuration,
        evidence,
        ..SearchRequest::new(
            &request.collection,
            &request.question,
            request.version.as_deref(),
            request.budget.into(),
        )
    };
    let input = search(&context.search, &search_request)
        .await
        .map_err(AskError::Search)?;
    let plan = AnswerPlan {
        min_rerank_score: configuration.min_rerank_score,
        top_rerank_score: top_rerank_score(&input.ranked),
        answer_prompt,
    };
    let bundle = assemble_evidence(context.search.database.clone(), input, counter)
        .await
        .map_err(AskError::Evidence)?;
    answer_relevant(
        context.port,
        request,
        context.answerer.as_ref(),
        bundle,
        plan,
    )
    .await
}

/// How `ask` answers one bundle: the reranker's top score for its search, the
/// least one `ask` answers from, and the prompt it answers with.
#[derive(Debug, Clone, Copy)]
pub(super) struct AnswerPlan<'prompt> {
    /// The configured threshold, when one is set.
    pub(super) min_rerank_score: Option<f32>,
    /// The top reranker score, absent when rerank did not run.
    pub(super) top_rerank_score: Option<f64>,
    /// The prompt the answerer is given above the threshold.
    pub(super) answer_prompt: &'prompt AnswerPrompt,
}

impl AnswerPlan<'_> {
    /// Whether rerank ran and its top score is below the threshold.
    fn is_below_threshold(self) -> bool {
        self.min_rerank_score
            .zip(self.top_rerank_score)
            .is_some_and(|(min, top)| top < f64::from(min))
    }
}

/// As [`answer_bundle`], but refuses with `no_evidence`, without chat, when
/// `plan` is below its threshold.
pub(super) async fn answer_relevant<P: ModelPort + Sync>(
    port: &P,
    request: &AskRequest,
    answerer: Option<&RegisteredAnswerer>,
    bundle: Bundle,
    plan: AnswerPlan<'_>,
) -> Result<Answer, AskError> {
    if plan.is_below_threshold() {
        let language = speaking(request, plan.answer_prompt);
        let message = below_threshold_message(language.texts);
        let response = ResponseContext::new(request, &bundle, answerer, language)?;
        return Ok(response.refused(RefusalCode::NoEvidence, message));
    }
    answer_bundle(port, request, answerer, bundle, plan.answer_prompt).await
}

/// Validates caller bounds without imposing language-detection rules.
fn validate_request(request: &AskRequest) -> Result<(), AskError> {
    if request.collection.trim().is_empty() {
        return Err(AskError::InvalidRequest("collection must not be blank"));
    }
    if request.question.trim().is_empty() || request.question.len() > 8192 {
        return Err(AskError::InvalidRequest(
            "question must contain 1 to 8192 UTF-8 bytes",
        ));
    }
    if request
        .version
        .as_ref()
        .is_some_and(|version| version.is_empty() || version.len() > 256)
    {
        return Err(AskError::InvalidRequest(
            "version must contain 1 to 256 UTF-8 bytes",
        ));
    }
    if RouterEntry::parse(&request.model).is_err() {
        return Err(AskError::InvalidRequest("model must be one router entry"));
    }
    if !(1..=RequestBudget::MAX_EVIDENCE_BUDGET).contains(&request.budget.evidence_bytes) {
        const _: () = assert!(RequestBudget::MAX_EVIDENCE_BUDGET == 24_000);
        return Err(AskError::InvalidRequest(
            "evidence_bytes must be between 1 and 24000",
        ));
    }
    if !request.budget.is_within_limits() {
        return Err(AskError::InvalidRequest(
            "ask budget is outside accepted limits",
        ));
    }
    Ok(())
}

/// The language a response speaks: its host-owned texts', and the tag its
/// `lang` reports.
#[derive(Debug, Clone)]
struct Speaking {
    /// The host-owned text language: en/fr/es, with English fallback.
    texts: ResponseLanguage,
    /// The tag `lang` reports.
    tag: String,
}

/// The explicit language `answer_prompt` presents, else the question's.
fn speaking(request: &AskRequest, answer_prompt: &AnswerPrompt) -> Speaking {
    if let Some(tag) = &answer_prompt.presentation().language {
        let texts = match tag.split('-').next() {
            Some("fr") => ResponseLanguage::French,
            Some("es") => ResponseLanguage::Spanish,
            _ => ResponseLanguage::English,
        };
        return Speaking {
            texts,
            tag: tag.clone(),
        };
    }
    let texts = response_language(request);
    Speaking {
        texts,
        tag: texts.code().to_owned(),
    }
}

/// Best-effort response metadata; an unknown question language is not a refusal.
fn response_language(request: &AskRequest) -> ResponseLanguage {
    match understand(&request.question).language {
        Language::French => ResponseLanguage::French,
        Language::English | Language::Unknown => ResponseLanguage::English,
    }
}

/// Runs answer generation over one already-verified evidence bundle, with
/// `answer_prompt`.
pub(super) async fn answer_bundle<P: ModelPort + Sync>(
    port: &P,
    request: &AskRequest,
    answerer: Option<&RegisteredAnswerer>,
    bundle: Bundle,
    answer_prompt: &AnswerPrompt,
) -> Result<Answer, AskError> {
    validate_request(request)?;
    let language = speaking(request, answer_prompt);
    let response = ResponseContext::new(request, &bundle, answerer, language)?;
    if bundle.passages.is_empty() {
        return Ok(response.refusal(RefusalCode::NoEvidence));
    }
    let Some(answerer) = answerer else {
        return Ok(response.refusal(RefusalCode::AnswererUnavailable));
    };
    check_answerer(request, answerer)?;

    let reply_cap = reply_cap(&answerer.card, request.budget.output_tokens);
    let mut messages = prompt(request, &bundle, answer_prompt)?;
    let mut rejections = Vec::new();
    for attempt in 1..=2 {
        let chat = chat_request(answerer, messages.clone(), reply_cap);
        let result = timeout(CHAT_DEADLINE, port.chat(&answerer.card, Room::Free, &chat))
            .await
            .map_err(|_| AskError::TimedOut)?;
        let (reply, invalid) = match result {
            Ok(reply) => match validate_reply(&reply, request, &bundle) {
                Ok(Reply::NotFound) => {
                    let refusal = response.refusal(RefusalCode::NotFound);
                    return Ok(explained(refusal, rejections, reply_cap));
                }
                Ok(Reply::Answer(valid)) => {
                    return Ok(explained(response.answer(valid)?, rejections, reply_cap));
                }
                Err(invalid) => (reply, invalid),
            },
            Err(GatewayError::InvalidAnswer { reason }) => (
                String::new(),
                Invalid {
                    failure: ValidationFailure::InvalidAnswer,
                    tokens: vec![reason],
                },
            ),
            Err(error) => return Err(AskError::Backend(error)),
        };
        repair(&mut messages, reply, invalid.failure);
        rejections.push(Rejection {
            attempt,
            check: invalid.failure.code(),
            tokens: invalid.tokens,
        });
    }
    let refusal = response.refusal(RefusalCode::Unsupported);
    Ok(explained(refusal, rejections, reply_cap))
}

/// Attaches the attempts rejected before `answer` and the `reply_cap` its
/// chat calls ran with, for a local explanation.
fn explained(answer: Answer, rejections: Vec<Rejection>, reply_cap: u32) -> Answer {
    Answer {
        rejections,
        reply_cap: Some(reply_cap),
        ..answer
    }
}

/// Adds one typed repair message without rerunning retrieval or changing evidence.
fn repair(messages: &mut Vec<Message>, reply: String, failure: ValidationFailure) {
    messages.push(Message {
        speaker: Speaker::Assistant,
        content: reply,
    });
    messages.push(Message {
        speaker: Speaker::User,
        content: format!(
            "Repair the answer once. Validation code: {}. Follow the original \
             evidence-only instructions.",
            failure.code()
        ),
    });
}

/// Checks the registry identity and card output ceiling before chat.
fn check_answerer(request: &AskRequest, answerer: &RegisteredAnswerer) -> Result<(), AskError> {
    if answerer.card.fields().role != Role::Answerer
        || answerer.card.fields().router_entry.as_str() != request.model
    {
        return Err(AskError::InvalidRequest(
            "registered answerer does not match the requested model entry",
        ));
    }
    if answerer
        .card
        .fields()
        .limits
        .output_tokens
        .zip(request.budget.output_tokens)
        .is_some_and(|(limit, requested)| requested > limit.get())
    {
        return Err(AskError::InvalidRequest(
            "output limit exceeds the registered answerer card",
        ));
    }
    Ok(())
}

/// Host-resolved metadata used to render a validated answer or refusal.
struct ResponseContext<'a> {
    /// The original request, copied into the public answer.
    request: &'a AskRequest,
    /// The verified evidence bundle that bounds the answer.
    bundle: &'a Bundle,
    /// The response language.
    language: Speaking,
    /// The registered answerer identity, when one is available.
    answerer: Option<&'a RegisteredAnswerer>,
    /// Source-resolved citation metadata indexed by passage number.
    passages: BTreeMap<u32, AnswerCitation>,
    /// Host-selected nearby passage metadata for a refusal.
    closest: Vec<AnswerCitation>,
}

impl<'a> ResponseContext<'a> {
    /// Builds citation and closest-passage metadata from the assembled bundle.
    fn new(
        request: &'a AskRequest,
        bundle: &'a Bundle,
        answerer: Option<&'a RegisteredAnswerer>,
        language: Speaking,
    ) -> Result<Self, AskError> {
        let passages = citation_metadata(bundle)?;
        let closest = passages.values().take(CLOSEST_LIMIT).cloned().collect();
        Ok(Self {
            request,
            bundle,
            language,
            answerer,
            passages,
            closest,
        })
    }

    /// Builds a host-written refusal without returning passage text.
    fn refusal(&self, code: RefusalCode) -> Answer {
        self.refused(code, refusal_message(code, self.language.texts))
    }

    /// Builds a refusal with `code` and the host-written `message`.
    fn refused(&self, code: RefusalCode, message: &str) -> Answer {
        Answer {
            schema: ANSWER_SCHEMA.to_owned(),
            collection: self.bundle.collection.clone(),
            generation: self.bundle.generation,
            question: self.request.question.clone(),
            lang: self.language.tag.clone(),
            answer: String::new(),
            citations: Vec::new(),
            model: model_metadata(self.request, self.answerer),
            uncalibrated: true,
            refusal: Some(AnswerRefusal {
                code,
                message: message.to_owned(),
            }),
            closest: self.closest.clone(),
            rejections: Vec::new(),
            routes: self.bundle.routes.clone(),
            delivered: self.bundle.passages.iter().map(Anchor::from).collect(),
            reply_cap: None,
            language_check: LanguageCheck::Unchecked,
        }
    }

    /// Resolves validated passage numbers to source-owned citation metadata.
    fn answer(&self, valid: ValidReply) -> Result<Answer, AskError> {
        let citations = valid
            .citations
            .iter()
            .map(|number| {
                self.passages
                    .get(number)
                    .cloned()
                    .ok_or(AskError::EvidenceIntegrity)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Answer {
            schema: ANSWER_SCHEMA.to_owned(),
            collection: self.bundle.collection.clone(),
            generation: self.bundle.generation,
            question: self.request.question.clone(),
            lang: self.language.tag.clone(),
            answer: valid.text,
            citations,
            model: model_metadata(self.request, self.answerer),
            uncalibrated: true,
            refusal: None,
            closest: Vec::new(),
            rejections: Vec::new(),
            routes: self.bundle.routes.clone(),
            delivered: self.bundle.passages.iter().map(Anchor::from).collect(),
            reply_cap: None,
            language_check: LanguageCheck::Unchecked,
        })
    }
}

/// Copies citation metadata from the exact passages and their assembly trace.
fn citation_metadata(bundle: &Bundle) -> Result<BTreeMap<u32, AnswerCitation>, AskError> {
    bundle
        .passages
        .iter()
        .map(|passage| {
            let chunk_id = bundle
                .trace
                .iter()
                .find(|trace| trace.n == passage.n)
                .and_then(|trace| {
                    trace
                        .chunk_ids
                        .first()
                        .or_else(|| trace.parent_context_of.first())
                })
                .cloned()
                .ok_or(AskError::EvidenceIntegrity)?;
            Ok((
                passage.n,
                AnswerCitation {
                    n: passage.n,
                    chunk_id,
                    section_id: passage.section_id.clone(),
                    source_ref: passage.source_ref.clone(),
                    title: passage.title.clone(),
                    section_path: passage.section_path.clone(),
                    span: [passage.span.start, passage.span.end],
                },
            ))
        })
        .collect()
}

/// Associates the response with the registered card, never model output.
fn model_metadata(request: &AskRequest, answerer: Option<&RegisteredAnswerer>) -> AnswerModel {
    AnswerModel {
        router_entry: request.model.clone(),
        card_id: answerer.map(|registered| registered.id.clone()),
    }
}

/// Host-owned refusal text in the response language.
fn refusal_message(code: RefusalCode, language: ResponseLanguage) -> &'static str {
    match (code, language) {
        (RefusalCode::NotFound, ResponseLanguage::English) => {
            "The available passages do not answer the question."
        }
        (RefusalCode::NotFound, ResponseLanguage::French) => {
            "Les passages disponibles ne répondent pas à la question."
        }
        (RefusalCode::Unsupported, ResponseLanguage::English) => {
            "The answer could not be verified against the available evidence."
        }
        (RefusalCode::Unsupported, ResponseLanguage::French) => {
            "La réponse n’a pas pu être vérifiée à partir des éléments disponibles."
        }
        (RefusalCode::AnswererUnavailable, ResponseLanguage::English) => {
            "No registered answerer is available for the requested model."
        }
        (RefusalCode::AnswererUnavailable, ResponseLanguage::French) => {
            "Aucun modèle de réponse enregistré n’est disponible pour ce modèle."
        }
        (RefusalCode::NotFound, ResponseLanguage::Spanish) => {
            "Los pasajes disponibles no responden a la pregunta."
        }
        (RefusalCode::Unsupported, ResponseLanguage::Spanish) => {
            "La respuesta no pudo verificarse con la evidencia disponible."
        }
        (RefusalCode::AnswererUnavailable, ResponseLanguage::Spanish) => {
            "No hay un modelo de respuesta registrado disponible para el modelo solicitado."
        }
        (RefusalCode::NoEvidence, ResponseLanguage::Spanish) => {
            "Ningún pasaje coincide con la pregunta."
        }
        (RefusalCode::NoEvidence, ResponseLanguage::English) => "No passage matched the question.",
        (RefusalCode::NoEvidence, ResponseLanguage::French) => {
            "Aucun passage ne correspond à la question."
        }
    }
}

/// Host-owned text of a refusal whose best passage was below the relevance
/// threshold.
const fn below_threshold_message(language: ResponseLanguage) -> &'static str {
    match language {
        ResponseLanguage::English => "The best passage was below the relevance threshold.",
        ResponseLanguage::French => "Le meilleur passage est sous le seuil de pertinence.",
        ResponseLanguage::Spanish => "El mejor pasaje está por debajo del umbral de relevancia.",
    }
}
