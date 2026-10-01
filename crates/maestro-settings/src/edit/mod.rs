//! Editing a preferences file in place: where its keys are, and the set and
//! unset built on that.

mod change;
mod document;

pub use change::{EditError, set_in_document, unset_in_document};
