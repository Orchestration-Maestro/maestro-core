//! Versioned schema and byte contracts shared by writers, verifiers, and readers.
//!
//! Content rows use `E` (family, relation, subject ID, object ID, claim ID, collection,
//! generation) and `F` (claim ID, subject, predicate, literal type, lexeme,
//! collection, generation, claim collection, subject kind, subject name), followed
//! by the complete claim record: conditions, version/world validity, provenance,
//! ordered supports, review state and recorded time. Every string is UTF-8 with a
//! u32 big-endian byte length; numbers in fields are canonical decimal strings.
//! Conditions have a u32 big-endian count then sorted key/value fields. Validity
//! has a byte tag (0 unknown, 1 bounded); bounded values carry start/end options
//! (0 absent, 1 present followed by a string). Provenance is extractor/profile.
//! Supports have a u32 big-endian count then revision/block/start/end/quote-digest
//! fields in recorded order. Review is unreviewed/accepted/rejected/flagged.
//! Rows sort by encoded bytes; the SHA-256 input is the
//! `maestro-projection-content/2` tag, u64 big-endian row count, then rows.
//! Receipt names hash length-delimited fields: the `maestro-projection-name/1` tag,
//! collection, decimal generation, and claim-set ID; the
//! basename is `g` plus 64 lowercase hex digits plus `.lbdb`. Fixed-width
//! dot-free stems cannot be prefixes or native companion stems of each other.

/// Projection schema and required indexes for the typed-edge records.
pub(super) const SCHEMA_VERSION: &str = "maestro-typed-edges/1";

/// Logical access paths an adapter proves from its durable catalog before publication.
/// Native edges use entity primary keys plus typed adjacency; facts use the entity
/// primary key and complete subject properties, not invented secondary indexes.
pub(super) const REQUIRED_INDEXES: &[&str] =
    &["edge_by_scope_family_source", "fact_by_scope_subject"];
