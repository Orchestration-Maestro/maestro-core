//! Tests of claims: admitting a verified set or nothing, reading it back,
//! verifying each support against its revision's original bytes, and
//! building and attaching sets under a job's lease.

mod attachments;
mod build_atomicity;
mod builds;
mod claims;
mod schema;
mod support;
mod supports;
mod upgrade;
mod vocabulary;

mod projection;
mod projection_guards;
mod projection_lease;
mod projection_pins;
mod resolution;
mod resolution_guards;

mod mutation_contracts;
