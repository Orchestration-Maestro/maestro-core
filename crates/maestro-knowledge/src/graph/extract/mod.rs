//! Bounded model extraction over canonical source windows.

mod run;
mod windows;

pub use run::ModelExtractor;
pub use windows::{Window, WindowPolicy, WindowPolicyError, locate_quote, windows};

#[cfg(test)]
mod tests;
