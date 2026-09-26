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
/// The model port is asynchronous and a `TokenCounter` is not: the port's
/// calls run on a thread of the tokenizer's own, which a caller may reach
/// from a plain thread or from inside a runtime.
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
    /// in free room, and any other refusal of the port.
    pub fn qualify<P>(port: P, card: ModelCard) -> Result<Self, TokenizerError>
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
        let bridge = Bridge::start(port, card)?;
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

/// The chunker's refusal for `error`: its text, since a count's refusal
/// holds no type of its own.
fn refusal(error: &TokenizerError) -> Error {
    Error(error.to_string())
}

impl TokenCounter for RouterTokenizer {
    /// `router/1:sha256:<the card's digest>`.
    fn contract_id(&self) -> &str {
        &self.contract_id
    }

    /// Tokenizes the canary fixtures again, as the router does now.
    ///
    /// # Errors
    ///
    /// Refuses the first canary whose IDs changed since qualification, and
    /// any refusal of the port.
    fn verify(&self) -> Result<(), Error> {
        agree(&self.bridge, &self.canaries).map_err(|error| refusal(&error))
    }

    /// The router's ordered token IDs for the complete `input`, special
    /// tokens included, without padding or truncation.
    ///
    /// # Errors
    ///
    /// Any refusal of the port, an embedder that does not fit in free room
    /// among them.
    fn token_ids(&self, input: &str) -> Result<Vec<u32>, Error> {
        self.bridge.tokenize(input).map_err(|error| refusal(&error))
    }
}
