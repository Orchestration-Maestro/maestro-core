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
mod knowledge_publish;
mod knowledge_verify_recheck;
mod machine;
mod quality_gates;
mod setup_installs;
mod status_summaries;
mod support;
