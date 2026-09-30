//! Versioned schema and byte contracts shared by writers, verifiers, and readers.
//!
//! Content rows use `E` (family, relation, subject ID, object ID, claim ID, collection,
//! generation) and `F` (claim ID, subject, predicate, literal type, lexeme,
//! collection, generation). Each field is UTF-8 preceded by a u32 big-endian
//! byte length. Rows sort by encoded bytes; the SHA-256 input is the
//! `maestro-projection-content/1` tag, u64 big-endian row count, then rows.
//! Receipt names hash length-delimited fields: the `maestro-projection-name/1` tag,
//! collection, decimal generation, and claim-set ID; the
//! basename is `g` plus 64 lowercase hex digits plus `.lbdb`. Fixed-width
//! dot-free stems cannot be prefixes or native companion stems of each other.

/// Projection schema and required indexes for the typed-edge records.
pub(super) const SCHEMA_VERSION: &str = "maestro-typed-edges/1";

/// Stable index names an adapter verifies before publication.
pub(super) const REQUIRED_INDEXES: &[&str] =
    &["edge_by_scope_family_source", "fact_by_scope_subject"];
