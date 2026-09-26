//! The crate's integration tests, built as one test crate: each module proves
//! one contract of the public API.

mod collection_contract;
mod corpus_contract;
mod eval_synthetic;
mod import_contract;
mod lexical_accents;
mod lexical_fold;
mod lexical_golden;
mod lexical_rules;
mod lexical_sample;
mod lexical_vectors;
mod live_router;
mod local_collection;
mod prepare_live;
mod publish_live;
mod qdrant_projection;
mod quality_gate;
mod quality_ledger;
mod router_parity;
mod suite_check;
mod suite_contract;
mod suite_resolution;
mod synthetic_collection;
