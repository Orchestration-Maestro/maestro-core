//! Tests of the router tokenizer: qualification by parity with the native
//! counter, counting and verifying through a port that answers the native
//! goldens, the router client against a stub router, the parity fixtures and
//! the refusals.

mod counting;
mod parity;
mod port;
mod qualification;
mod refusals;
mod router_client;
mod stub;
mod support;
