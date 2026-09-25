//! Structural preparation and packing; only the public native path certifies counts.
pub(crate) use drafts::build_drafts;
pub(crate) use limits::{MAX_TOKENS, TARGET_TOKENS};
pub(crate) use replay::validate_preparation;

mod context;
mod drafts;
mod layout;
mod limits;
mod prepare;
mod refusal;
mod replay;
mod structure;
#[cfg(test)]
mod tests;
