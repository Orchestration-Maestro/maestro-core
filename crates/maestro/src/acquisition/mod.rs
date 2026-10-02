//! Native acquisition's local authority boundary.
pub(crate) mod authority;
#[cfg(target_os = "linux")]
mod authority_host;
#[cfg(target_os = "linux")]
mod authority_probe;
#[cfg(target_os = "linux")]
mod authority_service;
#[cfg(target_os = "linux")]
mod authority_store;
#[cfg(test)]
mod flow_edges;
#[cfg(test)]
mod flow_tests;
#[cfg(test)]
mod history_tests;
#[cfg(test)]
mod record_tests;
#[cfg(test)]
mod resume_tests;
#[cfg(test)]
mod sync_reader_tests;

mod bindings;
pub(crate) mod cli;
// Live admission/transport composition is Linux-only; injected ports are tested everywhere.
#[cfg(any(target_os = "linux", test))]
mod command;
#[cfg(any(target_os = "linux", test))]
mod controls;
mod inspect;
mod output;
mod resources;
#[cfg(any(target_os = "linux", test))]
mod sync_budget;
#[cfg(any(target_os = "linux", test))]
mod sync_capture;
#[cfg(any(target_os = "linux", test))]
mod sync_discovery;
#[cfg(any(target_os = "linux", test))]
mod sync_disposition;
#[cfg(any(target_os = "linux", test))]
mod sync_source;
