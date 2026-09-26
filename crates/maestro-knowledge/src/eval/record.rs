//! Recording a report in the kernel, under the collection, generation and
//! suite it names itself.

use super::report::Report;
use maestro_kernel::{
    eval::{self, NewReport},
    store::Database,
};
use std::{error, fmt};

/// Why a report could not be recorded in the kernel.
#[derive(Debug)]
pub enum RecordError {
    /// The report could not be written as JSON.
    Json(serde_json::Error),
    /// The kernel refused the report, or could not record it.
    Kernel(eval::Error),
}

impl fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(formatter, "the report could not be written: {error}"),
            Self::Kernel(error) => write!(formatter, "the report could not be recorded: {error}"),
        }
    }
}

impl error::Error for RecordError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Json(error) => Some(error),
            Self::Kernel(error) => Some(error),
        }
    }
}

/// Records `report` in `database` as the kernel records every report
/// ([`Database::record_eval_report`]): its JSON as an artifact, indexed by
/// the collection, generation and suite the report itself names, so that the
/// record never disagrees with the JSON it pins.
///
/// # Errors
///
/// [`RecordError::Json`] when the report cannot be written as JSON, and
/// [`RecordError::Kernel`] when the kernel refuses it, for a generation its
/// collection does not record, or cannot record it.
pub fn record(database: &Database, report: &Report) -> Result<eval::Report, RecordError> {
    let json = serde_json::to_vec(report).map_err(RecordError::Json)?;
    database
        .record_eval_report(&NewReport {
            collection_id: &report.collection,
            generation: report.generation,
            suite: &report.suite,
            json: &json,
        })
        .map_err(RecordError::Kernel)
}
