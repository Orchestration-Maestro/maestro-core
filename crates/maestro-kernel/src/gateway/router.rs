//! The router client: the model port over maestro-model-router's dedicated
//! endpoints, `/models/<entry>/…`.

use super::{
    card::{ModelCard, Role},
    port::{Error, Message, ModelPort, Room, embedder_dimensions, require},
};
use crate::artifact::Digest;
use reqwest::{Client, RequestBuilder, Url};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    sync::{Mutex, PoisonError},
};

/// The request header that lets the router load a model only into free room
/// (T002): with its one value, `free`, the router refuses rather than unload
/// another model.
const ROOM_HEADER: &str = "X-Model-Router-Room";

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
    /// `http://127.0.0.1:8080`. It uses no proxy and reads no environment
    /// variable, and it sets no deadline: its caller does.
    ///
    /// # Errors
    ///
    /// [`Error::Transport`] when the HTTP client cannot be built.
    pub fn new(base: Url) -> Result<Self, Error> {
        let http = Client::builder()
            .no_proxy()
            .build()
            .map_err(Error::Transport)?;
        Ok(Self {
            base,
            http,
            checked: Mutex::default(),
        })
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
        let props: Props = send(self.http.get(self.endpoint(card, "props")), room).await?;
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
    /// checked.
    async fn call<T: DeserializeOwned>(
        &self,
        card: &ModelCard,
        room: Room,
        path: &str,
        body: &Value,
    ) -> Result<T, Error> {
        self.check(card, room).await?;
        send(self.http.post(self.endpoint(card, path)).json(body), room).await
    }
}

impl ModelPort for RouterClient {
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
        let answer: Embeddings = self
            .call(card, room, "v1/embeddings", &json!({"input": inputs}))
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
        let answer: Ranking = self.call(card, room, "v1/rerank", &body).await?;
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
        let answer: Tokens = self.call(card, room, "tokenize", &body).await?;
        Ok(answer.tokens)
    }

    async fn chat(
        &self,
        card: &ModelCard,
        room: Room,
        messages: &[Message],
    ) -> Result<String, Error> {
        require(card, Role::Answerer)?;
        let body = json!({"messages": messages});
        let answer: Completion = self.call(card, room, "v1/chat/completions", &body).await?;
        answer
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .ok_or_else(|| invalid("no reply"))
    }
}

/// Sends `request` in `room`, and reads its answer as `T`.
async fn send<T: DeserializeOwned>(request: RequestBuilder, room: Room) -> Result<T, Error> {
    let request = match room {
        Room::Free => request.header(ROOM_HEADER, "free"),
        // No header at all: the router refuses any value but `free`.
        Room::Any => request,
    };
    let response = request.send().await.map_err(Error::Transport)?;
    let status = response.status();
    let body = response.bytes().await.map_err(Error::Transport)?;
    if !status.is_success() {
        return Err(refusal(status.as_u16(), &body));
    }
    serde_json::from_slice(&body).map_err(|error| invalid(error.to_string()))
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
    /// The replies; one unless more were asked for.
    choices: Vec<Choice>,
}

/// One reply.
#[derive(Deserialize)]
struct Choice {
    /// The reply's message.
    message: Reply,
}

/// A reply's message.
#[derive(Deserialize)]
struct Reply {
    /// The text, which a reply that only calls tools lacks.
    content: Option<String>,
}
