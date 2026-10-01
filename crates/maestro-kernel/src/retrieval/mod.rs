//! Controlled, scope-bound retrieval and its shared literal search rules.

mod error;
mod identifiers;
mod inventory;
mod read;
#[cfg(test)]
mod tests;
mod types;
mod write;

pub use error::Error;
pub use identifiers::{contains_identifier, normalize_whitespace};
pub use types::{
    ChunkHit, Clock, IDENTIFIER_PROFILE, IdentifierSearchResult, InventoryRequest,
    InventorySelection, ReadControl, SearchInput, SearchMember, SearchProjection, SearchRead,
    SystemClock,
};
