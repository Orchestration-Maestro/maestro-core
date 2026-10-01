//! Evaluation reports (plan D13; FR-S1-009): each run of an evaluation suite
//! over a generation of a collection, kept as the kernel keeps every record.
//!
//! A report's JSON, `maestro-eval-report/1`, whose contract the knowledge
//! crate owns, is stored as an artifact, and
//! [`Database::record_eval_report`](crate::store::Database::record_eval_report)
//! records it in one write: its row in `eval_reports`, which names the
//! collection, the generation and the suite and pins the artifact, and its
//! event, [`RECORDED`], on the collection's stream, `collection/<id>`, in the
//! collection's scope. A report is recorded only for a generation of its own
//! collection, and keeps that generation's id once the generation's row is
//! gone: a measurement outlives what it measured. The table refuses to
//! change, replace or delete a report, whoever writes.
//!
//! Reports are read, as every record of the kernel is, through the caller's
//! `ScopeSet`: [`Database::eval_report`](crate::store::Database::eval_report)
//! and [`Database::eval_reports`](crate::store::Database::eval_reports) see
//! only the reports of the collections whose scope the set covers.

mod error;
mod report;
#[cfg(test)]
mod tests;

pub use error::Error;
pub use report::{NewReport, RECORDED, Report};
