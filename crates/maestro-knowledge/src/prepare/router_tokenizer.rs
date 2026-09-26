//! The router tokenizer: maestro-canonicalization's `TokenCounter` over the
//! model router's `/tokenize` for one embedder's card, qualified by parity
//! with the native counter.

use super::{
    bridge::Bridge,
    error::TokenizerError,
    parity::{Fixture, fixtures},
};
use maestro_canonicalization::{Error, TokenCounter};
use maestro_kernel::gateway::{ModelCard, ModelPort, Role};
use std::time::Duration;

/// How long one call to the port may take: long enough for the router to
/// load the embedder into free room from cold, then tokenize.
const DEADLINE: Duration = Duration::from_secs(60);

/// Counts tokens as one embedder counts them, through the model router's
/// `/tokenize` for the embedder's model card (ADR-0008).
///
/// A router tokenizer exists only once qualified: [`RouterTokenizer::qualify`]
/// is its one constructor, and it refuses the router at the first parity
/// fixture of the native profile whose IDs differ from the native counter's.
/// Every call asks for free room, so counting never unloads another model
/// (FR-S1-015a).
///
/// Its contract ID is `router/1:sha256:<the card's digest>`. The card pins
/// the model file and the router's llama.cpp build, `server_build`, so a new
/// card or a new build gives a new contract ID, and with it a new chunk set.
/// `verify` tokenizes the canary fixtures again, so a model or a build that
/// changes under the same card is refused although the gateway checks a card
/// only once.
///
/// Through `TokenCounter`, a refusal is text. [`RouterTokenizer::check`] and
/// [`RouterTokenizer::count`] are `verify` and `token_ids` with their refusals
/// typed: when `chunk_documents` refuses a batch, `check` tells an embedder
/// that does not fit in free room for now, [`TokenizerError::Unavailable`],
/// from a model or a build that changed under the card,
/// [`TokenizerError::Disagreement`].
///
/// The model port is asynchronous and a `TokenCounter` is not: the port's
/// calls run on a thread of the tokenizer's own, each within 60 s, so that
/// none waits forever on a router that never answers. A call blocks its
/// caller until the answer or the deadline. It never panics inside a
/// runtime, but it holds that runtime's thread: from async code, call it
/// through `tokio::task::spawn_blocking`.
#[derive(Debug)]
pub struct RouterTokenizer {
    /// `router/1:sha256:<the card's digest>`.
    contract_id: String,
    /// The fixtures `verify` tokenizes again.
    canaries: Vec<Fixture>,
    /// The port's `tokenize` for the card.
    bridge: Bridge,
}

impl RouterTokenizer {
    /// The router tokenizer of the embedder `card` names, through `port`,
    /// once the port gives every parity fixture of the native profile the
    /// native counter's ordered IDs.
    ///
    /// # Errors
    ///
    /// [`TokenizerError::NotAnEmbedder`] for any other card, before any call;
    /// [`TokenizerError::Disagreement`] for the first fixture whose IDs
    /// differ; [`TokenizerError::Unavailable`] when the embedder does not fit
    /// in free room; [`TokenizerError::TimedOut`] for a call the port did not
    /// answer within 60 s, and any other refusal of the port.
    pub fn qualify<P>(port: P, card: ModelCard) -> Result<Self, TokenizerError>
    where
        P: ModelPort + Send + 'static,
    {
        Self::qualify_within(port, card, DEADLINE)
    }

    /// [`RouterTokenizer::qualify`], with each call to the port within
    /// `deadline` rather than [`DEADLINE`], which tests cannot wait out.
    pub(super) fn qualify_within<P>(
        port: P,
        card: ModelCard,
        deadline: Duration,
    ) -> Result<Self, TokenizerError>
    where
        P: ModelPort + Send + 'static,
    {
        let role = card.fields().role;
        if role != Role::Embedder {
            return Err(TokenizerError::NotAnEmbedder {
                card: card.digest().clone(),
                role,
            });
        }
        let fixtures = fixtures().map_err(TokenizerError::Fixtures)?;
        let contract_id = format!("router/1:sha256:{}", card.digest().as_str());
        let bridge = Bridge::start(port, card, deadline)?;
        agree(&bridge, &fixtures)?;
        Ok(Self {
            contract_id,
            canaries: fixtures
                .into_iter()
                .filter(|fixture| fixture.canary)
                .collect(),
            bridge,
        })
    }

    /// Tokenizes the canary fixtures again, as the router does now: `verify`,
    /// with its refusal typed.
    ///
    /// # Errors
    ///
    /// [`TokenizerError::Disagreement`] for the first canary whose IDs
    /// changed since qualification; [`TokenizerError::Unavailable`] when the
    /// embedder does not fit in free room; [`TokenizerError::TimedOut`] and
    /// any other refusal of the port.
    pub fn check(&self) -> Result<(), TokenizerError> {
        agree(&self.bridge, &self.canaries)
    }

    /// The router's ordered token IDs for the complete `input`, as given,
    /// special tokens included, without padding or truncation: `token_ids`,
    /// with its refusal typed.
    ///
    /// # Errors
    ///
    /// [`TokenizerError::Unavailable`] when the embedder does not fit in free
    /// room; [`TokenizerError::TimedOut`] and any other refusal of the port.
    pub fn count(&self, input: &str) -> Result<Vec<u32>, TokenizerError> {
        self.bridge.tokenize(input)
    }
}

/// Refuses the first of `fixtures` whose IDs through `bridge` differ from
/// the native counter's.
fn agree(bridge: &Bridge, fixtures: &[Fixture]) -> Result<(), TokenizerError> {
    for fixture in fixtures {
        let router = bridge.tokenize(&fixture.input)?;
        if router != fixture.ids {
            return Err(TokenizerError::Disagreement {
                fixture: fixture.name.clone(),
                input: fixture.input.clone(),
                native: fixture.ids.clone(),
                router,
            });
        }
    }
    Ok(())
}

/// The chunker's refusal for `error`: its text, since the chunker's error
/// holds text only; `check` and `count` keep the type.
fn refusal(error: &TokenizerError) -> Error {
    Error(error.to_string())
}

impl TokenCounter for RouterTokenizer {
    /// `router/1:sha256:<the card's digest>`.
    fn contract_id(&self) -> &str {
        &self.contract_id
    }

    /// [`RouterTokenizer::check`], its refusal as text.
    ///
    /// # Errors
    ///
    /// Refuses the first canary whose IDs changed since qualification, and
    /// any refusal of the port.
    fn verify(&self) -> Result<(), Error> {
        self.check().map_err(|error| refusal(&error))
    }

    /// [`RouterTokenizer::count`], its refusal as text.
    ///
    /// # Errors
    ///
    /// Any refusal of the port, an embedder that does not fit in free room
    /// and a call past the deadline among them.
    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        self.count(input).map_err(|error| refusal(&error))
    }
}
