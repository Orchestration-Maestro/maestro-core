//! Shared address admission, pinned TLS, robots, origin pacing and resource budgets.
pub(crate) mod accounting;
pub mod address;
pub mod budget;
pub mod connect;
pub mod dns;
mod failure;
mod floor;
pub mod http;
mod http_protocol;
pub mod pacing;
pub mod robots;
pub mod robots_store;
pub mod stream;
mod wire_quota;

#[cfg(test)]
mod http_history_tests;

#[cfg(test)]
mod mutation_tests;
