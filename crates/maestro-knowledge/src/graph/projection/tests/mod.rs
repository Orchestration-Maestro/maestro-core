//! Contracts of the public graph projection port and backend-neutral writer.

mod content_fields;
pub(in crate::graph::projection) mod contract;
pub(in crate::graph::projection) mod contract_reads;
mod port;
mod projection_writer;
