//! Rule-based understanding of normalized queries, identifiers, language, and kind.

mod identifier_numbers;
mod identifier_patterns;
mod identifier_types;
mod identifiers;
mod kind;
mod language;
mod normalize;
#[cfg(test)]
mod tests;
mod understand;

pub use identifier_types::{Family, Identifier};
pub use kind::QueryKind;
pub use language::Language;
pub use understand::{Understood, understand};
