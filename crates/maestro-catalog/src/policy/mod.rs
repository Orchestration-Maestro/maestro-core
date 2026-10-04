//! Effect-free Cedar checking and separately supplied trusted host facts.

mod check;
pub(crate) mod schema;
#[cfg(test)]
mod tests;
pub mod workspace;

pub use check::{Cedar, PolicyChecker, load};
pub use schema::{
    Case, Check, Decision, HostFacts, NoFacts, Operation, TrustedFacts, read_input, test_cases,
};
