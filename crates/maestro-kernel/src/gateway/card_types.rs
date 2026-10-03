//! Shared model-card vocabulary, independent of v1 and v2 encodings.

use crate::artifact::{self, Digest};
use schemars::JsonSchema;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::{
    error, fmt,
    num::{NonZeroU32, NonZeroUsize},
};

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

impl Role {
    /// Every role, in the order cards name them.
    pub const ALL: [Self; 3] = [Self::Embedder, Self::Reranker, Self::Answerer];
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

impl Serialize for RouterEntry {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RouterEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::parse(&name).map_err(de::Error::custom)
    }
}

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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

/// Common facts projected from a model card for existing model ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardFields {
    /// The role the model fills.
    pub role: Role,
    /// The router's name for the model.
    pub router_entry: RouterEntry,
    /// The SHA-256 of the model file; the router reports none, so no call checks it.
    pub file_digest: Digest,
    /// The SHA-256 of the chat template the server reports, where the card records one.
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

/// Why a card could not be recorded or loaded.
#[derive(Debug)]
pub enum CardError {
    /// The card is not a valid v1 or v2 model card; the text says why.
    Invalid(String),
    /// The artifact store could not keep or return the card.
    Store(artifact::Error),
}

impl fmt::Display for CardError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(reason) => write!(
                formatter,
                "not a valid maestro-model-card/1 or maestro-model-card/2 model card: {reason}"
            ),
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
