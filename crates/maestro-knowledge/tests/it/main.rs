//! The crate's integration tests, built as one test crate: each module proves
//! one contract of the public API.

mod answer_live;
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
mod n26_factor_s1_mapped_ingestion_without_changing_corpus_1;
mod n26_revision_links;
mod n26_support;
mod prepare_live;
mod publish_live;
pub(crate) mod qdrant_projection;
mod quality_gate;
mod quality_ledger;
mod router_parity;
mod search_live;
mod suite_check;
mod suite_contract;
mod suite_resolution;
mod synthetic_collection;
mod synthetic_gate;
