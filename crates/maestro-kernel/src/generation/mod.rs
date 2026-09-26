//! The search generations of a collection (building block B6; plan D9): each
//! one build of the collection's search projection from one chunk set, bound
//! to its embedding and sparse profiles.
//!
//! A generation is created `building`, then moves only to `verified`,
//! `published` and `retired`, in that order; any other move is refused,
//! naming both states. At most one generation of a collection is published,
//! which the database itself holds with a partial unique index, and
//! publishing one retires the one published before it in the same
//! transaction. A generation's id is never given twice, even once its row is
//! gone, so the projection named after it never names another generation.

mod error;
mod lifecycle;
mod state;
#[cfg(test)]
mod tests;

pub use error::Error;
pub use lifecycle::{Generation, NewGeneration};
pub use state::GenerationState;
