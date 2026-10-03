//! Native projection operations; only feature-enabled builds compile this door.

#[expect(
    dead_code,
    reason = "E08b wires the private native backend into the public lifecycle"
)]
pub(super) mod backend;
mod open;
mod reader;
mod rows;
mod schema;
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
