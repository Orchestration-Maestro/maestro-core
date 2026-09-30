//! Typed restrictive resolution over the canonical S1 settings descriptors.

mod resolve;

pub use resolve::{Layer, ResolveDiagnostic, ResolvedSettings, ResolvedValue, resolve};

#[cfg(test)]
mod tests;
