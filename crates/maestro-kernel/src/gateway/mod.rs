//! The model gateway (building block B10): every model, embedder, reranker,
//! tokenizer or answerer, is reached through a model card, never by a bare
//! model name (D8).
//!
//! One port, [`ModelPort`], has two implementations: [`RouterClient`], which
//! calls maestro-model-router's dedicated endpoints, and [`FakeModels`], the
//! deterministic stand-in public CI uses, since it has no GPU. The port is
//! async because a search runs its routes in parallel under a deadline (D10);
//! the caller sets that deadline.

mod card;
pub(crate) mod card_types;
pub mod card_v2;
mod fake;
mod port;
mod router;
#[cfg(test)]
mod tests;

pub use card::{CardError, CardFields, Limits, ModelCard, Role, RouterEntry, SuiteResult};
pub use card_v2::CardIdentity;
pub use fake::FakeModels;
pub use port::{ChatRequest, Error, MAX_CHAT_OUTPUT_TOKENS, Message, ModelPort, Room, Speaker};
pub use reqwest::Url;
pub use router::RouterClient;
