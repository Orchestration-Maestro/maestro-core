//! Strict versioned report contracts and their wire types.

mod base;
mod metrics;
mod v2;

pub use base::{Expected, Failure, FailureClass, Header, HeaderV2, QuestionResult, Report, Schema};
pub use metrics::{Estimate, Metrics};
pub use v2::{
    CohortStatistics, ItemStatus, MeasurementCohort, Subgroup, SubgroupStatistics, TrialMode,
};
