use super::{
    AnswerCitation, AnswerPrompt, AskBudget, AskError, AskRequest, PromptText, PromptVersion,
    RefusalCode, RegisteredAnswerer, Rejection,
};
use super::{
    generate::{AnswerPlan, answer_bundle, answer_relevant},
    prompt::prompt,
};
use maestro_kernel::{
    artifact::{Digest, Store},
    evidence::{Budget, Bundle, Passage, RequestBudget, RouteStatus, Schema, Span, Trace},
    gateway::{
        CardIdentity, ChatRequest, Error, ModelCard, ModelPort, Room, RouterEntry, Speaker,
        card_v2::{Capability, ControlValue, Sampling, SamplingParameters},
    },
};
use maestro_test_scratch::scratch_directory;
#[expect(
    dead_code,
    reason = "the shared v2 fixture also contains an unused embedder card"
)]
#[path = "../../../maestro-kernel/src/gateway/tests/v2_golden.rs"]
mod v2_golden;

use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    future::{self, Future},
    path::PathBuf,
    sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

struct ScriptedPort {
    replies: Mutex<VecDeque<Result<String, Error>>>,
    calls: AtomicUsize,
    chat_calls: Mutex<Vec<(Digest, Room, ChatRequest)>>,
    hangs: bool,
}

impl ScriptedPort {
    fn new(replies: &[&str]) -> Self {
        Self::with_results(
            replies
                .iter()
                .map(|reply| Ok((*reply).to_owned()))
                .collect(),
        )
    }

    /// Scripts both successful model text and gateway failures.
    fn with_results(replies: Vec<Result<String, Error>>) -> Self {
        Self {
            replies: Mutex::new(replies.into()),
            calls: AtomicUsize::new(0),
            chat_calls: Mutex::new(Vec::new()),
            hangs: false,
        }
    }

    /// Creates a port that holds the chat future open until its caller times out.
    fn never_resolves() -> Self {
        Self {
            replies: Mutex::new(VecDeque::new()),
            calls: AtomicUsize::new(0),
            chat_calls: Mutex::new(Vec::new()),
            hangs: true,
        }
    }
}

impl ModelPort for ScriptedPort {
    fn embed(
        &self,
        _card: &ModelCard,
        _room: Room,
        _inputs: &[String],
    ) -> impl Future<Output = Result<Vec<Vec<f32>>, Error>> + Send {
        future::ready(Err(Error::InvalidRequest {
            reason: "embedding is not used by this test".to_owned(),
        }))
    }

    fn rerank(
        &self,
        _card: &ModelCard,
        _room: Room,
        _query: &str,
        _documents: &[String],
    ) -> impl Future<Output = Result<Vec<f64>, Error>> + Send {
        future::ready(Err(Error::InvalidRequest {
            reason: "reranking is not used by this test".to_owned(),
        }))
    }

    fn tokenize(
        &self,
        _card: &ModelCard,
        _room: Room,
        _text: &str,
    ) -> impl Future<Output = Result<Vec<u32>, Error>> + Send {
        future::ready(Ok(Vec::new()))
    }

    fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        request: &ChatRequest,
    ) -> impl Future<Output = Result<String, Error>> + Send {
        self.calls.fetch_add(1, Ordering::Relaxed);
        self.chat_calls.lock().expect("chat-call lock").push((
            card.digest().clone(),
            room,
            request.clone(),
        ));
        let reply = if self.hangs {
            None
        } else {
            Some(
                self.replies
                    .lock()
                    .expect("reply lock")
                    .pop_front()
                    .unwrap_or_else(|| {
                        Err(Error::InvalidAnswer {
                            reason: "scripted reply exhausted".to_owned(),
                        })
                    }),
            )
        };
        async move {
            match reply {
                Some(reply) => reply,
                None => future::pending::<Result<String, Error>>().await,
            }
        }
    }
}

struct Scratch(PathBuf);

impl Scratch {
    fn answerer(&self) -> RegisteredAnswerer {
        self.answerer_with_output_limit(1024)
    }

    /// An answerer whose card allows `limit` output tokens.
    fn answerer_with_output_limit(&self, limit: u32) -> RegisteredAnswerer {
        self.answerer_for_model("qwen3-4b", limit, false)
    }

    fn answerer_for_model(&self, entry: &str, limit: u32, thinking: bool) -> RegisteredAnswerer {
        let mut card_json: serde_json::Value =
            serde_json::from_str(v2_golden::CANONICAL_ANSWERER_CARD)
                .expect("canonical v2 answerer card");
        card_json["identity"]["invocation"]["limits"]["output_tokens"] = serde_json::json!(limit);
        let mut identity: CardIdentity =
            serde_json::from_value(card_json["identity"].take()).expect("v2 identity");
        identity.router_entry = RouterEntry::parse(entry).expect("router entry");
        identity.invocation.reasoning = Capability::Supported(BTreeMap::from([(
            "enable_thinking".to_owned(),
            ControlValue::Boolean(thinking),
        )]));
        if entry == "ask-gemma4-e4b-nonthinking" {
            identity.invocation.sampling = Sampling::Configured(SamplingParameters {
                temperature: 1.0,
                top_p: 0.95,
                top_k: 64,
                min_p: 0.0,
                typical_p: 1.0,
                repeat_penalty: 1.0,
                frequency_penalty: 0.0,
                presence_penalty: 0.0,
                seed: Some(0),
            });
        }
        let card = ModelCard::record_v2(&Store::new(&self.0), &identity).expect("answerer v2 card");
        RegisteredAnswerer {
            id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned(),
            card,
        }
    }

    fn new() -> Self {
        Self(scratch_directory().unwrap())
    }
}

/// A v2 answerer card whose template sets `enable_thinking` to `thinking`,
/// recorded in a store that is gone once it is made.
pub(crate) fn reasoning_answerer(thinking: bool) -> ModelCard {
    let scratch = Scratch::new();
    let mut card = scratch.answerer().card;
    if thinking {
        let mut identity = card.identity().expect("v2 identity").clone();
        identity.invocation.reasoning = Capability::Supported(BTreeMap::from([(
            "enable_thinking".to_owned(),
            ControlValue::Boolean(true),
        )]));
        card = ModelCard::record_v2(&Store::new(&scratch.0), &identity).expect("v2 card");
    }
    card
}

impl Drop for Scratch {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove scratch card store");
    }
}

fn request(question: &str) -> AskRequest {
    AskRequest {
        collection: "docs".to_owned(),
        question: question.to_owned(),
        model: "qwen3-4b".to_owned(),
        version: None,
        budget: AskBudget::default(),
    }
}

fn bundle(question: &str, language: &str, text: &str) -> Bundle {
    let span = Span {
        start: 0,
        end: text.len(),
    };
    Bundle {
        schema: Schema::V1,
        collection: "docs".to_owned(),
        generation: 1,
        query: question.to_owned(),
        lang: language.to_owned(),
        routes: BTreeMap::from([("bm25".to_owned(), RouteStatus::Ok)]),
        passages: vec![Passage {
            n: 1,
            section_id: Some("section-1".to_owned()),
            document_id: "document-1".to_owned(),
            revision_id: "revision-1".to_owned(),
            title: "Using the collection".to_owned(),
            section_path: vec!["Commands".to_owned()],
            version: None,
            source_ref: "https://example.org/docs".to_owned(),
            span,
            digest: Digest::of(text.as_bytes()),
            text: text.to_owned(),
            windowed: false,
            alternates: Vec::new(),
        }],
        conflicts: Vec::new(),
        known_gaps: Vec::new(),
        budget: Budget {
            evidence_bytes: u32::try_from(text.len()).expect("small test passage"),
            limit: 6000,
            counter: Some("evidence-utf8-bytes/1".to_owned()),
            estimated: true,
        },
        request_budget: Some(RequestBudget::default()),
        inventory: None,
        trace: vec![Trace {
            parent_context_of: Vec::new(),
            n: 1,
            score: None,
            routes: vec!["bm25".to_owned()],
            chunk_ids: vec!["chunk-1".to_owned()],
            procedural: false,
        }],
    }
}

fn empty_bundle(question: &str) -> Bundle {
    let mut bundle = bundle(question, "en", "unused passage");
    bundle.passages.clear();
    bundle.trace.clear();
    bundle
}

#[tokio::test]
async fn unsupported_command_is_retried_once_then_refused() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("How do I list the registered sources?");
    let bundle = bundle(
        &request.question,
        "en",
        "Run `maestro knowledge collections` to see registered sources.",
    );
    let bad = "Run `maestro collection remove --all` to remove every source. [1]";
    let port = ScriptedPort::new(&[bad, bad]);

    let answer = answer_bundle(
        &port,
        &request,
        Some(&answerer),
        bundle,
        &PromptVersion::V1.into(),
    )
    .await
    .expect("safe unsupported refusal");

    assert_eq!(port.calls.load(Ordering::Relaxed), 2);
    assert_eq!(answer.answer, "");
    assert!(answer.citations.is_empty());
    assert_eq!(
        answer.refusal.expect("refusal").code,
        RefusalCode::Unsupported
    );
}

#[tokio::test]
async fn numeric_prefix_inside_a_different_number_is_rejected_after_one_retry() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("Which default port does the service use?");
    let passage = bundle(
        &request.question,
        "en",
        "The service listens on port 8080 by default.",
    );
    let unsupported = "The service listens on port 80 by default. [1]";
    let port = ScriptedPort::new(&[unsupported, unsupported]);

    let answer = answer_bundle(
        &port,
        &request,
        Some(&answerer),
        passage,
        &PromptVersion::V1.into(),
    )
    .await
    .expect("safe unsupported refusal");

    assert_eq!(port.calls.load(Ordering::Relaxed), 2);
    assert_eq!(answer.answer, "");
    assert_eq!(
        answer.refusal.expect("refusal").code,
        RefusalCode::Unsupported
    );
}

#[tokio::test]
async fn citations_are_host_resolved_without_a_language_check() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("Comment configurer le service selon la documentation?");
    let passage = bundle(
        &request.question,
        "fr",
        "Le service utilise le port 8080 par défaut.",
    );
    let port = ScriptedPort::new(&["The service uses port 8080 by default. [1]"]);

    let answer = answer_bundle(
        &port,
        &request,
        Some(&answerer),
        passage,
        &PromptVersion::V1.into(),
    )
    .await
    .expect("validated answer");

    assert_eq!(port.calls.load(Ordering::Relaxed), 1);
    assert_eq!(answer.model.card_id.as_deref(), Some(answerer.id.as_str()));
    assert_eq!(answer.answer, "The service uses port 8080 by default. [1]");
    assert_eq!(
        answer.citations,
        [AnswerCitation {
            n: 1,
            chunk_id: "chunk-1".to_owned(),
            section_id: Some("section-1".to_owned()),
            source_ref: "https://example.org/docs".to_owned(),
            title: "Using the collection".to_owned(),
            section_path: vec!["Commands".to_owned()],
            span: [0, "Le service utilise le port 8080 par défaut.".len()],
        }]
    );

    let calls = port.chat_calls.lock().expect("chat-call lock");
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, *answerer.card.digest());
    assert_eq!(calls[0].1, Room::Free);
    let chat = &calls[0].2;
    assert_eq!(chat.max_output_tokens, 1024);
    assert_eq!(
        chat.chat_template_kwargs,
        BTreeMap::from([("enable_thinking".to_owned(), ControlValue::Boolean(false))])
    );
    assert_eq!(chat.messages[0].speaker, Speaker::System);
    assert!(
        chat.messages[0]
            .content
            .contains("Answer in the language of the question")
    );
    assert_eq!(chat.messages[1].speaker, Speaker::User);
    assert!(chat.messages[1].content.contains(&request.question));
    assert!(
        chat.messages[1]
            .content
            .contains("Le service utilise le port 8080 par défaut.")
    );
}

#[test]
fn ask_request_is_strict_and_defaults_to_the_bounded_local_model() {
    let request: AskRequest = serde_json::from_value(serde_json::json!({
        "collection": "docs",
        "question": "How is the service configured?"
    }))
    .expect("default ask request");
    assert_eq!(request.model, "ask-gemma4-e4b-nonthinking");
    assert_eq!(request.budget.evidence_bytes, 6_000);
    assert_eq!(request.budget.output_tokens, None);
    assert!(
        serde_json::from_value::<AskRequest>(serde_json::json!({
            "collection": "docs",
            "question": "How is the service configured?",
            "untrusted": true
        }))
        .is_err()
    );
}

#[tokio::test]
async fn i5_empty_search_result_refuses_without_generation() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("How does the service work?");
    let port = ScriptedPort::new(&[]);
    let answer = answer_bundle(
        &port,
        &request,
        Some(&answerer),
        empty_bundle(&request.question),
        &PromptVersion::V1.into(),
    )
    .await
    .expect("safe refusal");

    assert_eq!(port.calls.load(Ordering::Relaxed), 0);
    let refusal = answer.refusal.expect("refusal");
    assert_eq!(refusal.code, RefusalCode::NoEvidence);
    assert_eq!(refusal.message, "No passage matched the question.");
}

#[tokio::test]
async fn i2_short_undetected_question_reaches_the_answerer() {
    let scratch = Scratch::new();
    let answerer = scratch.answerer();
    let request = request("x");
    let port = ScriptedPort::new(&["The local service uses verified instructions. [1]"]);

    let answer = answer_bundle(
        &port,
        &request,
        Some(&answerer),
        bundle("x", "en", "The local service uses verified instructions."),
        &PromptVersion::V1.into(),
    )
    .await
    .expect("short question answer");

    assert_eq!(port.calls.load(Ordering::Relaxed), 1);
    assert!(answer.refusal.is_none());
}

#[path = "tests/delivered.rs"]
mod delivered;
#[path = "tests/explain.rs"]
mod explain;
#[path = "tests/guardrails.rs"]
mod guardrails;
#[path = "tests/presentation.rs"]
mod presentation;
#[path = "tests/prompt_text.rs"]
mod prompt_text;
#[path = "tests/prompts.rs"]
mod prompts;
#[path = "tests/reply_cap.rs"]
mod reply_cap;
#[path = "tests/requests.rs"]
mod requests;
#[path = "tests/router_refusal.rs"]
mod router_refusal;
#[path = "tests/threshold.rs"]
mod threshold;

#[path = "tests/parent_context.rs"]
mod parent_context;
