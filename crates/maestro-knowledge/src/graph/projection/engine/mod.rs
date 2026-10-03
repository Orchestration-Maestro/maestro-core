//! Native projection operations; only feature-enabled builds compile this door.

pub(super) mod backend;
mod cancellation;
mod config;
mod open;
pub(super) mod probe;
#[cfg(test)]
mod probe_files_tests;
#[cfg(test)]
mod probe_tests;
pub(super) mod producer;
mod reader;
pub(super) mod registry;
#[cfg(test)]
mod registry_tests;
mod rows;
mod schema;
#[cfg(test)]
mod settings_tests;
#[cfg(test)]
mod tests;
mod transaction;

#[cfg(test)]
#[cfg(not(windows))]
mod rollback_repro;

#[cfg(test)]
mod codec_tests;
#[cfg(test)]
#[cfg(not(windows))]
mod validation_tests;

#[cfg(test)]
mod backend_tests;

// Native Windows writes are refused; guard/identity and Unsupported removal are tested separately.
#[cfg(test)]
#[cfg(unix)]
pub(super) mod cleanup_tests;
#[cfg(test)]
mod public_fixture;
#[cfg(test)]
mod public_tests;

#[cfg(test)]
mod holder_fence_tests;
#[cfg(test)]
mod public_guard_tests;
