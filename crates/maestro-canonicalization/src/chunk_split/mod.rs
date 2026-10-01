//! Structural preparation and packing; only `chunk_documents`, through a verified `TokenCounter`,
//! certifies counts.
pub(crate) use chrome::heading_title;
pub(crate) use drafts::build_drafts;
pub(crate) use limits::{MAX_TOKENS, TARGET_TOKENS};
pub(crate) use replay::validate_preparation;
pub(crate) use structure::Layout;

mod chrome;
mod context;
mod drafts;
mod ideas;
mod layout;
mod limits;
mod prepare;
mod ranges;
mod refusal;
mod replay;
mod structure;
#[cfg(test)]
mod tests;
