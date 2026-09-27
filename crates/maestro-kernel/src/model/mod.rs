//! Scoped persistence for model cards, evaluations, and explicit selections.

mod error;
mod read;
mod records;
#[cfg(test)]
mod tests;
mod write;

pub use error::Error;
pub use records::{
    CardRecord, EvaluationDisposition, EvaluationMode, EvaluationRecord, NewModelCard,
    NewModelEvaluation, NewModelSelection, SelectedModelCard, SelectionRecord,
};
