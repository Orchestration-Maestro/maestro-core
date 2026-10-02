//! The command line's tests, built as one test crate: each runs the built
//! `maestro` in a scratch home with `std::process::Command`, as its users
//! and their agents do.
#![cfg(test)]

mod backup_restore;
mod backup_restore_targets;
mod cli_contract;
mod collection_status;
mod doctor_checks;
#[cfg(unix)]
mod fakes;
mod graph_build;
mod graph_eval;
mod graph_extract;
mod graph_operations;
mod graph_resume;
mod import_jobs;
mod job_waits;
mod knowledge_ask;
mod knowledge_collections;
mod knowledge_get;
mod knowledge_prepare_profiles;
mod knowledge_prepare_v2;
mod knowledge_publish;
mod knowledge_verify_recheck;
mod machine;
mod mcp_clients;
mod mcp_stdio;
mod model_cli;
mod model_cli_registration;
mod model_cli_selection;
mod publish_again;
mod quality_gates;
mod rebuild_drill;
mod settings_config;
mod setup_installs;
mod status_summaries;
mod support;

mod graph_draft;
mod graph_draft_bounds;
mod graph_draft_redirect;
