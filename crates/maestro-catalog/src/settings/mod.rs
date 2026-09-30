//! Restrictive catalog settings classes and typed resolution over the S1 registry.

mod classes;
mod resolve;

pub use classes::{SettingClassError, SettingClasses};
pub use resolve::{ResolveDiagnostic, ResolvedSettings, ResolvedValue, resolve};

#[cfg(test)]
mod tests;
