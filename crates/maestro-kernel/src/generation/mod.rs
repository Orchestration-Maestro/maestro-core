//! The search generations of a collection (building block B6; plan D9): each
//! one build of the collection's search projection from one of its chunk
//! sets, bound to its embedding and sparse profiles.
//!
//! A generation is created `building`, then moves only to `verified`,
//! `published` and `retired`, in that order, or from `building` or
//! `verified` to `failed`, which is final: a failed generation is never
//! published nor resumed. Any other move is refused, naming both states. At
//! most one generation of a collection is published, which the database
//! itself holds with a partial unique index, and publishing one retires the
//! one published before it in the same transaction, and no other. A
//! generation's id is never given twice, even once its row is gone, so the
//! projection named after it never names another generation. A generation
//! has its collection's scope, and every reader takes the caller's
//! [`ScopeSet`](crate::scope::ScopeSet), which must cover it.

mod error;
mod lifecycle;
mod state;
#[cfg(test)]
mod tests;

pub use error::Error;
pub use lifecycle::{Generation, NewGeneration};
pub use state::GenerationState;
