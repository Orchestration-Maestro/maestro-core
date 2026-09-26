//! Token counting: the `TokenCounter` seam, and the qualified local executable that fills it.
mod artifacts;
mod binding;
mod contract;
mod counter;
mod loader;
mod native;
mod process;
#[cfg(test)]
mod tests;

pub use counter::TokenCounter;
pub use native::NativeTokenizer;
