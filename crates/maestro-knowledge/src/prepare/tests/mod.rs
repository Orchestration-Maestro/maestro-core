//! Tests of the preparation: the router tokenizer, qualified by parity with
//! the native counter, counting and verifying through a port that answers
//! the native goldens, the router client against a stub router, the parity
//! fixtures and the refusals; then the preparation of a collection over it,
//! from its eligible revisions to a complete chunk set, with its duplicates,
//! its refusals and its interruptions.

mod chunk_sets;
mod counting;
mod duplicates;
mod eligibility;
mod interruptions;
mod latest;
mod near;
mod oversized;
mod parity;
mod port;
mod qualification;
mod qualification_validation;
mod refusals;
mod router_client;
pub(crate) mod scratch;
mod section_reader;
mod stops;
mod stub;
pub(crate) mod support;
mod synthetic;
