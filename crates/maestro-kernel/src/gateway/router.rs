//! The router client: the model port over maestro-model-router's dedicated
//! endpoints, `/models/<entry>/…`.

use super::{
    body::{
        MAX_CATALOG_BODY_BYTES, MAX_CHAT_BODY_BYTES, MAX_ERROR_BODY_BYTES, MAX_PROPS_BODY_BYTES,
        embeddings_limit, ranking_limit, read_bounded, tokens_limit,
    },
    card::{ModelCard, Role, RouterEntry},
    port::{ChatRequest, Error, ModelPort, Room, embedder_dimensions, require},
};
use crate::artifact::Digest;
use crate::json::canonical;
use reqwest::{Client, RequestBuilder, Url, redirect::Policy};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Map, Value, json};
use std::{
    collections::HashSet,
    sync::{Mutex, PoisonError},
};

/// The request header that lets the router load a model only into free room
/// (T002): with its one value, `free`, the router refuses rather than unload
/// another model.
const ROOM_HEADER: &str = "X-Model-Router-Room";
/// Maximum decoded answer content accepted from the model.
const MAX_CHAT_CONTENT_BYTES: usize = 32_768;
// A reply at the chat ceiling fits the content cap at 16 bytes per token.
const _: () = assert!(super::port::MAX_CHAT_OUTPUT_TOKENS as usize * 16 <= MAX_CHAT_CONTENT_BYTES);

/// A client of the model router. Each call goes to the entry its card names,
/// after the card is checked against what the model's server reports, and
/// in the room the call names: a call in [`Room::Free`] asks for free room
/// and never unloads another model (FR-S1-015a), while a call in
/// [`Room::Any`] names no room, so the router may unload an idle model for it.
#[derive(Debug)]
pub struct RouterClient {
    /// Where the router answers.
    base: Url,
    /// The HTTP client, which keeps connections to the router open.
    http: Client,
    /// The cards that passed their check against their server's `/props`.
    checked: Mutex<HashSet<Digest>>,
}

impl RouterClient {
    /// A client of the router answering at the root of `base`, such as
    /// `http://127.0.0.1:8080`. It uses no proxy, follows no redirect and
    /// reads no environment variable, and it sets no deadline: its caller
    /// does.
    ///
    /// # Errors
    ///
    /// [`Error::Transport`] when the HTTP client cannot be built.
    pub fn new(base: Url) -> Result<Self, Error> {
        let http = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .build()
            .map_err(Error::Transport)?;
        Ok(Self {
            base,
            http,
            checked: Mutex::default(),
        })
    }

    /// The entries of the router's catalog, in its order, as `GET
    /// /v1/models` lists them: the listing starts no model, so it asks for
    /// no room.
    ///
    /// # Errors
    ///
    /// [`Error::Transport`] when the router cannot be reached,
    /// [`Error::Refused`] when it refuses, and [`Error::InvalidAnswer`] when
    /// the answer is no listing or names an entry that is no entry name.
    pub async fn catalog(&self) -> Result<Vec<RouterEntry>, Error> {
        let mut url = self.base.clone();
        url.set_path("/v1/models");
        let listing: Listing = send(self.http.get(url), Room::Any, MAX_CATALOG_BODY_BYTES).await?;
        listing
            .data
            .into_iter()
            .map(|model| {
                RouterEntry::parse(&model.id).map_err(|_| {
                    invalid(format!(
                        "the catalog lists {:?}, which is no entry name",
                        model.id
                    ))
                })
            })
            .collect()
    }

    /// The URL of `path` under the model `card` names.
    fn endpoint(&self, card: &ModelCard, path: &str) -> Url {
        let mut url = self.base.clone();
        url.set_path(&format!(
            "/models/{}/{path}",
            card.fields().router_entry.as_str()
        ));
        url
    }

    /// Checks `card` against what its model's server reports, its build and,
    /// where the card records one, its chat template, asking `/props` in the
    /// room of the call the check precedes.
    ///
    /// A card counts as checked only once it passes: a refused card, or one
    /// whose check the router refused, is checked again at its next call.
    /// Calls that start together before a card first passes may each check it.
    async fn check(&self, card: &ModelCard, room: Room) -> Result<(), Error> {
        if self.is_checked(card.digest()) {
            return Ok(());
        }
        self.recheck(card, room).await
    }

    /// Asks `/props` in `room` and compares `card` with it, checked or not:
    /// the request reloads a model the router unloaded.
    async fn recheck(&self, card: &ModelCard, room: Room) -> Result<(), Error> {
        let request = self.http.get(self.endpoint(card, "props"));
        let props: Props = send(request, room, MAX_PROPS_BODY_BYTES).await?;
        props.compare(card)?;
        self.checked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(card.digest().clone());
        Ok(())
    }

    /// Whether the card under `digest` was checked.
    fn is_checked(&self, digest: &Digest) -> bool {
        self.checked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(digest)
    }

    /// Posts `body` to `path` under the model `card` names, once the card is
    /// checked, and reads an answer of at most `limit` bytes.
    async fn call<T: DeserializeOwned>(
        &self,
        card: &ModelCard,
        room: Room,
        (path, limit): (&str, usize),
        body: &Value,
    ) -> Result<T, Error> {
        self.check(card, room).await?;
        let request = self
            .http
            .post(self.endpoint(card, path))
            .json(&canonical(body.clone()));
        send(request, room, limit).await
    }
}

impl ModelPort for RouterClient {
    /// Checks the card on every call, which loads its model in `room` when
    /// it is not loaded, also after the router unloaded it while idle: a
    /// long-lived client, such as the MCP server's, reloads the model here
    /// rather than in the call's window. The calls keep their cached check.
    async fn prepare(&self, card: &ModelCard, room: Room) -> Result<(), Error> {
        self.recheck(card, room).await
    }

    async fn embed(
        &self,
        card: &ModelCard,
        room: Room,
        inputs: &[String],
    ) -> Result<Vec<Vec<f32>>, Error> {
        let dimensions = embedder_dimensions(card)?;
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let limit = embeddings_limit(inputs.len(), dimensions.get());
        let answer: Embeddings = self
            .call(
                card,
                room,
                ("v1/embeddings", limit),
                &json!({"input": inputs}),
            )
            .await?;
        let vectors = by_index(
            inputs.len(),
            answer
                .data
                .into_iter()
                .map(|item| (item.index, item.embedding)),
        )?;
        match vectors
            .iter()
            .find(|vector| vector.len() != dimensions.get())
        {
            Some(vector) => Err(invalid(format!(
                "a vector of {} dimensions, where the card records {dimensions}",
                vector.len()
            ))),
            None => Ok(vectors),
        }
    }

    async fn rerank(
        &self,
        card: &ModelCard,
        room: Room,
        query: &str,
        documents: &[String],
    ) -> Result<Vec<f64>, Error> {
        require(card, Role::Reranker)?;
        if documents.is_empty() {
            return Ok(Vec::new());
        }
        let body = json!({"query": query, "documents": documents});
        let limit = ranking_limit(documents.len());
        let answer: Ranking = self.call(card, room, ("v1/rerank", limit), &body).await?;
        by_index(
            documents.len(),
            answer
                .results
                .into_iter()
                .map(|result| (result.index, result.relevance_score)),
        )
    }

    async fn tokenize(&self, card: &ModelCard, room: Room, text: &str) -> Result<Vec<u32>, Error> {
        // As the embedding path counts: special tokens added and parsed, as
        // maestro-canonicalization's TOKENIZER.md records.
        let body = json!({"content": text, "add_special": true, "parse_special": true});
        let limit = tokens_limit(text.len());
        let answer: Tokens = self.call(card, room, ("tokenize", limit), &body).await?;
        Ok(answer.tokens)
    }

    async fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        request: &ChatRequest,
    ) -> Result<String, Error> {
        let sampling = request.validate(card)?;
        let mut body = Map::from_iter([
            ("messages".to_owned(), json!(request.messages)),
            ("max_tokens".to_owned(), json!(request.max_output_tokens)),
            ("stream".to_owned(), Value::Bool(false)),
            (
                "chat_template_kwargs".to_owned(),
                request.template_values()?,
            ),
        ]);
        if let Some(sampling) = sampling {
            for (field, value) in [
                ("temperature", json!(sampling.temperature)),
                ("top_p", json!(sampling.top_p)),
                ("top_k", json!(sampling.top_k)),
                ("min_p", json!(sampling.min_p)),
                ("typical_p", json!(sampling.typical_p)),
                ("repeat_penalty", json!(sampling.repeat_penalty)),
                ("frequency_penalty", json!(sampling.frequency_penalty)),
                ("presence_penalty", json!(sampling.presence_penalty)),
            ] {
                body.insert(field.to_owned(), value);
            }
            if let Some(seed) = sampling.seed {
                body.insert("seed".to_owned(), json!(seed));
            }
        }
        let answer: Completion = self
            .call(
                card,
                room,
                ("v1/chat/completions", MAX_CHAT_BODY_BYTES),
                &Value::Object(body),
            )
            .await?;
        answer.into_content()
    }
}

/// Sends `request` in `room`, and reads its answer as `T` when it is at most
/// `limit` bytes, and a refusal when it is at most [`MAX_ERROR_BODY_BYTES`];
/// a longer refusal keeps its status and quotes only the limit.
/// A redirect is refused unread: following it could send the request's body
/// off this machine.
async fn send<T: DeserializeOwned>(
    request: RequestBuilder,
    room: Room,
    limit: usize,
) -> Result<T, Error> {
    let request = match room {
        Room::Free => request.header(ROOM_HEADER, "free"),
        // No header at all: the router refuses any value but `free`.
        Room::Any => request,
    };
    let mut response = request.send().await.map_err(Error::Transport)?;
    let status = response.status();
    if status.is_redirection() {
        return Err(Error::Redirected {
            status: status.as_u16(),
        });
    }
    if !status.is_success() {
        let status = status.as_u16();
        return Err(
            match read_bounded(&mut response, MAX_ERROR_BODY_BYTES).await {
                Ok(body) => refusal(status, &body),
                // A refusal too long to read is still a refusal of its status,
                // not a model answer to repair: only the limit is quoted.
                Err(Error::InvalidAnswer { reason }) => Error::Refused {
                    status,
                    code: None,
                    message: reason,
                },
                Err(error) => error,
            },
        );
    }
    let body = read_bounded(&mut response, limit).await?;
    serde_json::from_slice(&body).map_err(|error| invalid(error.to_string()))
}

/// The text of a one-choice, complete assistant reply.
fn completion_content(completion: Completion) -> Result<String, Error> {
    let mut choices = completion.choices.into_iter();
    let Some(choice) = choices.next() else {
        return Err(invalid("chat response must contain exactly one choice"));
    };
    if choices.next().is_some() {
        return Err(invalid("chat response must contain exactly one choice"));
    }
    if choice.index != 0 {
        return Err(invalid("chat response choice index is not zero"));
    }
    if choice.finish_reason != "stop" {
        return Err(invalid("chat response was truncated or ended unexpectedly"));
    }
    if choice.message.role != "assistant" || has_tool_call(&choice.message) {
        return Err(invalid("chat response is not a plain assistant reply"));
    }
    let content = choice
        .message
        .content
        .filter(|content| !content.is_empty())
        .ok_or_else(|| invalid("chat response has no content"))?;
    if content.len() > MAX_CHAT_CONTENT_BYTES {
        return Err(invalid("chat response content exceeds 32768 bytes"));
    }
    Ok(content)
}

/// Whether a chat completion tries to call a tool or function.
fn has_tool_call(message: &Reply) -> bool {
    message.tool_calls.as_ref().is_some_and(has_value)
        || message.function_call.as_ref().is_some_and(has_value)
}

/// Whether a provider field contains a nonempty tool invocation.
fn has_value(value: &Value) -> bool {
    !value.is_null() && !value.as_array().is_some_and(Vec::is_empty)
}

/// The error a refusal of `status` with `body` means: the router's own
/// envelope, `{"error": {"code", "message", "type"}}`, when the body is one.
fn refusal(status: u16, body: &[u8]) -> Error {
    let (code, message) = match serde_json::from_slice::<Envelope>(body) {
        Ok(Envelope { error }) => (error.code.and_then(named), error.message),
        Err(_) => (None, String::from_utf8_lossy(body).into_owned()),
    };
    match (status, code.as_deref()) {
        (503, Some("insufficient_room")) => Error::Unavailable { reason: message },
        (400, Some("unknown_room")) => Error::UnknownRoom { message },
        _ => Error::Refused {
            status,
            code,
            message,
        },
    }
}

/// A refusal's code when it is in words, as the router's are; llama.cpp's
/// server repeats the status as a number instead.
fn named(code: Value) -> Option<String> {
    match code {
        Value::String(code) => Some(code),
        _ => None,
    }
}

/// Places each answer at its index: every input answered exactly once.
fn by_index<T>(
    count: usize,
    answers: impl IntoIterator<Item = (usize, T)>,
) -> Result<Vec<T>, Error> {
    let mut placed: Vec<Option<T>> = (0..count).map(|_| None).collect();
    for (index, answer) in answers {
        let slot = placed
            .get_mut(index)
            .filter(|slot| slot.is_none())
            .ok_or_else(|| {
                invalid(format!(
                    "answer {index} is outside the {count} inputs, or repeated"
                ))
            })?;
        *slot = Some(answer);
    }
    placed
        .into_iter()
        .collect::<Option<Vec<T>>>()
        .ok_or_else(|| invalid(format!("fewer answers than the {count} inputs")))
}

/// An [`Error::InvalidAnswer`] saying how.
fn invalid(reason: impl Into<String>) -> Error {
    Error::InvalidAnswer {
        reason: reason.into(),
    }
}

/// What the model's server reports of itself, in part.
#[derive(Deserialize)]
struct Props {
    /// llama.cpp's build.
    build_info: String,
    /// The chat template, which a server may not report.
    chat_template: Option<String>,
}

impl Props {
    /// Refuses `card` unless it records what the server reports.
    fn compare(&self, card: &ModelCard) -> Result<(), Error> {
        let fields = card.fields();
        let mismatch = |property, recorded: &str, reported: &str| Error::CardMismatch {
            card: card.digest().clone(),
            property,
            recorded: recorded.to_owned(),
            reported: reported.to_owned(),
        };
        if self.build_info != fields.server_build {
            return Err(mismatch(
                "build_info",
                &fields.server_build,
                &self.build_info,
            ));
        }
        let Some(recorded) = &fields.template_digest else {
            return Ok(());
        };
        let reported = self
            .chat_template
            .as_ref()
            .map(|template| Digest::of(template.as_bytes()));
        match reported {
            Some(digest) if digest == *recorded => Ok(()),
            other => Err(mismatch(
                "chat_template",
                recorded.as_str(),
                other.as_ref().map_or("none", Digest::as_str),
            )),
        }
    }
}

/// A refusal as the router, and llama.cpp's server, write it.
#[derive(Deserialize)]
struct Envelope {
    /// The refusal.
    error: Refusal,
}

/// The inside of a refusal's envelope.
#[derive(Deserialize)]
struct Refusal {
    /// Why, for the reader.
    message: String,
    /// Why, for a program: a word from the router, a number from llama.cpp.
    code: Option<Value>,
}

/// `/v1/models`' answer: the router's catalog.
#[derive(Deserialize)]
struct Listing {
    /// One model per entry.
    data: Vec<Listed>,
}

/// One entry of the catalog.
#[derive(Deserialize)]
struct Listed {
    /// Its name.
    id: String,
}

/// `/v1/embeddings`' answer.
#[derive(Deserialize)]
struct Embeddings {
    /// One vector per input.
    data: Vec<Embedded>,
}

/// One input's vector.
#[derive(Deserialize)]
struct Embedded {
    /// The input's place among the inputs.
    index: usize,
    /// The vector.
    embedding: Vec<f32>,
}

/// `/v1/rerank`'s answer, ordered by score.
#[derive(Deserialize)]
struct Ranking {
    /// One score per document.
    results: Vec<Ranked>,
}

/// One document's score.
#[derive(Deserialize)]
struct Ranked {
    /// The document's place among the documents.
    index: usize,
    /// The score.
    relevance_score: f64,
}

/// `/tokenize`'s answer.
#[derive(Deserialize)]
struct Tokens {
    /// The token IDs, in order.
    tokens: Vec<u32>,
}

/// `/v1/chat/completions`' answer.
#[derive(Deserialize)]
struct Completion {
    /// The choices returned for the single prompt.
    choices: Vec<Choice>,
}

impl Completion {
    /// The one plain reply, if the response did not truncate or call tools.
    fn into_content(self) -> Result<String, Error> {
        completion_content(self)
    }
}

/// One reply.
#[derive(Deserialize)]
struct Choice {
    /// Its position in the request's choices.
    index: usize,
    /// Why generation stopped.
    finish_reason: String,
    /// The reply's message.
    message: Reply,
}

/// A reply's message.
#[derive(Deserialize)]
struct Reply {
    /// The role the provider returned.
    role: String,
    /// The generated text, which a reply that only calls tools lacks.
    content: Option<String>,
    /// A provider tool invocation, which ask never permits.
    tool_calls: Option<Value>,
    /// A legacy provider function invocation, which ask never permits.
    function_call: Option<Value>,
}
