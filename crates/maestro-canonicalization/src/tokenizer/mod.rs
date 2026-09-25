//! Local vocabulary-only tokenization through the qualified executable.
mod artifacts;
mod binding;
mod contract;
mod native;
mod process;
#[cfg(test)]
mod tests;

pub use native::NativeTokenizer;
