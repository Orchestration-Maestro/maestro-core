//! The one immutable set of numeric limits every catalog input path shares
//! (plan D2, security limits): sources and preferences, the bundle writer and
//! reader, downloads and the verifier. Production entry points always pass
//! [`Limits::PRODUCTION`]; tests inject small values into the same code paths.

#[cfg(test)]
mod tests;
mod values;

pub use values::Limits;
