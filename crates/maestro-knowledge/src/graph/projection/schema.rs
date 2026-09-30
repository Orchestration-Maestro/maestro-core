//! Versioned schema identifiers shared by writers, verifiers, and readers.

/// Projection schema and required indexes for the typed-edge records.
pub(super) const SCHEMA_VERSION: &str = "maestro-typed-edges/1";

/// Stable index names an adapter verifies before publication.
pub(super) const REQUIRED_INDEXES: &[&str] =
    &["edge_by_scope_family_source", "fact_by_scope_subject"];
