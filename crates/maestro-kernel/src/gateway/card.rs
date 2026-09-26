//! Model cards: what was evaluated of a model filling a role (D8), kept as
//! strict JSON artifacts whose digest is the card's identity.
//!
//! A card is written as `maestro-model-card/1`:
//!
//! ```json
//! {
//!   "schema": "maestro-model-card/1",
//!   "role": "embedder",
//!   "router_entry": "embed",
//!   "file_digest": "<SHA-256 of the model file, recorded when the card is made>",
//!   "template_digest": null,
//!   "server_build": "b6500-3f2c9a1b",
//!   "dimensions": 1024,
//!   "limits": {"context_tokens": 8192, "output_tokens": null},
//!   "suite_results": [{"suite": "synthetic-retrieval", "report": "<SHA-256>"}]
//! }
//! ```
//!
//! An unknown or repeated key is refused, and so is a digest that is not 64
//! lowercase hexadecimal characters. `server_build` is llama.cpp's
//! `build_info` and `template_digest` the SHA-256 of the chat template, both
//! as the model's server reports them through `/props`; an embedder alone
//! records `dimensions`. `output_tokens` is null for a model that generates
//! nothing, and each suite result names the digest of its report.

use crate::artifact::{self, Digest, Store};
use serde::{Deserialize, Serialize};
use std::{
    error, fmt,
    num::{NonZeroU32, NonZeroUsize},
};

/// The only card schema this version reads and writes.
const CARD_SCHEMA: &str = "maestro-model-card/1";

/// The role a model fills, which decides what a gateway may ask of it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Turns text into vectors of the card's `dimensions`.
    Embedder,
    /// Scores documents against a query.
    Reranker,
    /// Answers a question from the evidence it is given.
    Answerer,
}

impl fmt::Display for Role {
    /// The role as a card writes it.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Embedder => "embedder",
            Self::Reranker => "reranker",
            Self::Answerer => "answerer",
        })
    }
}

/// A model's name in the router's catalog, as its dedicated endpoints take
/// it: `/models/<entry>/…`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RouterEntry(String);

impl RouterEntry {
    /// An entry from its name: ASCII letters, digits, `.`, `_` and `-`, and
    /// never `.` or `..` alone, so that the name is one path segment as
    /// written.
    ///
    /// # Errors
    ///
    /// [`CardError::Invalid`] naming the refused name.
    pub fn parse(name: &str) -> Result<Self, CardError> {
        let segment = name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
        if segment && !matches!(name, "" | "." | "..") {
            Ok(Self(name.to_owned()))
        } else {
            Err(CardError::Invalid(format!(
                "router_entry {name:?} is no catalog entry: ASCII letters, digits, \
                 '.', '_' and '-', never '.' or '..' alone"
            )))
        }
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What a model can take and give, as the bake-off measured it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    /// The most tokens the model reads at once.
    pub context_tokens: NonZeroU32,
    /// The most tokens one reply may hold; none for a model that generates
    /// nothing.
    pub output_tokens: Option<NonZeroU32>,
}

/// One suite the model was evaluated on, and the report it produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteResult {
    /// The suite's name, such as `synthetic-retrieval`.
    pub suite: String,
    /// The digest of the report, an artifact of its own.
    pub report: Digest,
}

/// What a card records (D8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardFields {
    /// The role the model fills.
    pub role: Role,
    /// The router's name for the model.
    pub router_entry: RouterEntry,
    /// The SHA-256 of the model file, recorded when the card is made; the
    /// router reports none, so no call checks it.
    pub file_digest: Digest,
    /// The SHA-256 of the chat template the server reports, where the card
    /// records one.
    pub template_digest: Option<Digest>,
    /// The llama.cpp build the server reports, its `build_info`.
    pub server_build: String,
    /// The size of an embedder's vectors; none for any other role.
    pub dimensions: Option<NonZeroUsize>,
    /// What the model can take and give.
    pub limits: Limits,
    /// The suites the model was evaluated on.
    pub suite_results: Vec<SuiteResult>,
}

/// A model card: what it records, identified by the digest of its JSON.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCard {
    /// The digest of the card's JSON, its identity.
    digest: Digest,
    /// What the card records.
    fields: CardFields,
}

impl ModelCard {
    /// Records `fields` in `store` as a new card and returns it: its JSON is
    /// checked as [`ModelCard::load`] checks it, then stored as an artifact.
    ///
    /// # Errors
    ///
    /// [`CardError::Invalid`] when the fields break a rule of the card, such
    /// as dimensions on a card that is not an embedder's, and
    /// [`CardError::Store`] when the store cannot keep the card.
    pub fn record(store: &Store, fields: &CardFields) -> Result<Self, CardError> {
        let json = serde_json::to_vec(&CardJson::of(fields)).map_err(invalid)?;
        let card = Self::from_json(&json)?;
        store.put(&json).map_err(CardError::Store)?;
        Ok(card)
    }

    /// The card stored under `digest`.
    ///
    /// # Errors
    ///
    /// [`CardError::Store`] when the store holds no intact artifact under
    /// `digest`, and [`CardError::Invalid`] when the artifact is not a valid
    /// card.
    pub fn load(store: &Store, digest: &Digest) -> Result<Self, CardError> {
        let json = store.get(digest).map_err(CardError::Store)?;
        Self::from_json(&json)
    }

    /// The card's identity: the digest of its JSON.
    #[must_use]
    pub fn digest(&self) -> &Digest {
        &self.digest
    }

    /// What the card records.
    #[must_use]
    pub fn fields(&self) -> &CardFields {
        &self.fields
    }

    /// The card whose JSON is `json`.
    fn from_json(json: &[u8]) -> Result<Self, CardError> {
        let written: CardJson = serde_json::from_slice(json).map_err(invalid)?;
        Ok(Self {
            digest: Digest::of(json),
            fields: written.into_fields()?,
        })
    }
}

/// Why a card could not be recorded or loaded.
#[derive(Debug)]
pub enum CardError {
    /// The card is not a valid `maestro-model-card/1`; the text says why.
    Invalid(String),
    /// The artifact store could not keep or return the card.
    Store(artifact::Error),
}

impl fmt::Display for CardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(reason) => {
                write!(formatter, "not a valid {CARD_SCHEMA} model card: {reason}")
            }
            Self::Store(_) => formatter.write_str("the model card could not be stored or read"),
        }
    }
}

impl error::Error for CardError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Store(source) => Some(source),
            Self::Invalid(_) => None,
        }
    }
}

/// A [`CardError::Invalid`] saying why.
fn invalid(reason: impl fmt::Display) -> CardError {
    CardError::Invalid(reason.to_string())
}

/// The digest `text` names, for the card's field `field`.
fn parse_digest(field: &str, text: &str) -> Result<Digest, CardError> {
    Digest::parse(text).map_err(|refused| invalid(format_args!("{field}: {refused}")))
}

/// A card as its JSON is written.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CardJson {
    /// Must equal [`CARD_SCHEMA`].
    schema: String,
    /// See [`CardFields::role`].
    role: Role,
    /// See [`CardFields::router_entry`].
    router_entry: String,
    /// See [`CardFields::file_digest`].
    file_digest: String,
    /// See [`CardFields::template_digest`].
    template_digest: Option<String>,
    /// See [`CardFields::server_build`].
    server_build: String,
    /// See [`CardFields::dimensions`].
    dimensions: Option<NonZeroUsize>,
    /// See [`CardFields::limits`].
    limits: Limits,
    /// See [`CardFields::suite_results`].
    suite_results: Vec<SuiteJson>,
}

impl CardJson {
    /// The JSON of `fields`.
    fn of(fields: &CardFields) -> Self {
        Self {
            schema: CARD_SCHEMA.to_owned(),
            role: fields.role,
            router_entry: fields.router_entry.as_str().to_owned(),
            file_digest: fields.file_digest.as_str().to_owned(),
            template_digest: fields
                .template_digest
                .as_ref()
                .map(|digest| digest.as_str().to_owned()),
            server_build: fields.server_build.clone(),
            dimensions: fields.dimensions,
            limits: fields.limits,
            suite_results: fields
                .suite_results
                .iter()
                .map(|result| SuiteJson {
                    suite: result.suite.clone(),
                    report: result.report.as_str().to_owned(),
                })
                .collect(),
        }
    }

    /// What the JSON records, once each of the card's rules holds.
    fn into_fields(self) -> Result<CardFields, CardError> {
        if self.schema != CARD_SCHEMA {
            return Err(invalid(format_args!(
                "schema {:?} is not {CARD_SCHEMA}",
                self.schema
            )));
        }
        match (self.role, self.dimensions) {
            (Role::Embedder, None) => return Err(invalid("an embedder's card records dimensions")),
            (Role::Reranker | Role::Answerer, Some(_)) => {
                return Err(invalid(format_args!(
                    "a {}'s card records no dimensions",
                    self.role
                )));
            }
            _ => {}
        }
        let suite_results = self
            .suite_results
            .into_iter()
            .map(|result| {
                Ok(SuiteResult {
                    report: parse_digest("report", &result.report)?,
                    suite: result.suite,
                })
            })
            .collect::<Result<_, CardError>>()?;
        Ok(CardFields {
            role: self.role,
            router_entry: RouterEntry::parse(&self.router_entry)?,
            file_digest: parse_digest("file_digest", &self.file_digest)?,
            template_digest: self
                .template_digest
                .map(|text| parse_digest("template_digest", &text))
                .transpose()?,
            server_build: self.server_build,
            dimensions: self.dimensions,
            limits: self.limits,
            suite_results,
        })
    }
}

/// A suite result as its JSON is written.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SuiteJson {
    /// See [`SuiteResult::suite`].
    suite: String,
    /// See [`SuiteResult::report`].
    report: String,
}
