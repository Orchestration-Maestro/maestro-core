//! The machine's checks (plan D14, FR-S1-015): the kernel's configuration,
//! database and artifact tree, the search service setup installs, the model
//! router and each role's model card. `maestro doctor` reports every check,
//! each failure with its next action, and `maestro status` summarizes which
//! services and collections are ready.

mod check;
pub(super) mod doctor;
mod findings;
mod kernel;
mod services;
mod settings;
pub(super) mod status;
#[cfg(test)]
mod tests;

pub(crate) use services::{
    DEFAULT_QDRANT, QDRANT_VARIABLE, ROUTER_VARIABLE, qdrant_url, router_url,
};
