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
mod n05_authority_contract;
#[cfg(target_os = "linux")]
mod n05_authority_frames;
#[cfg(target_os = "linux")]
mod n05_authority_linux;
mod n05_establish_owner_only_grants_and_the_read_only_authority_port;
mod publish_again;
mod quality_gates;
mod rebuild_drill;
mod settings_config;
mod setup_installs;
mod status_summaries;
mod support;
