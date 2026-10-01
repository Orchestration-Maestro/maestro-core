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
