//! Contracts of the public graph projection port and backend-neutral writer.

mod binding;
mod content_fields;
pub(in crate::graph::projection) mod contract;
pub(in crate::graph::projection) mod contract_reads;
mod import;
mod port;
mod projection_writer;

mod cleanup;
mod cleanup_support;

mod cleanup_boundaries;
#[cfg(unix)]
mod cleanup_process;
mod lifecycle;
