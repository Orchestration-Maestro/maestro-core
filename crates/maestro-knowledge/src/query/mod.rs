//! Rule-based understanding of normalized queries, identifiers, language, and kind.

mod identifier_numbers;
mod identifier_patterns;
mod identifier_types;
mod identifiers;
mod index;
mod kind;
mod language;
mod normalize;
#[cfg(test)]
mod tests;
mod understand;

pub use identifier_types::{Family, Identifier};
pub(crate) use index::index_identifiers;
pub use kind::QueryKind;
pub use language::Language;
pub use maestro_kernel::retrieval::IDENTIFIER_PROFILE as PROFILE;
pub(crate) use understand::{INVENTORY_DOCUMENT_FORMS, INVENTORY_FILTERS, INVENTORY_VERSION_FORMS};
pub use understand::{Understood, understand};
