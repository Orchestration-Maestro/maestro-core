//! The crate's integration tests, built as one test crate: each module proves
//! one contract of the public API or of the command-line tool.

mod chunk_contract;
mod chunk_native;
mod cli_contract;
mod dedup_contract;
mod dialect_properties;
mod document_contract;
mod phase_a_acceptance;
mod validation_boundary;
