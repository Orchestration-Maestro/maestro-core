//! Tests of the evaluation runner: how it ranks and judges each question,
//! the documents a question expects whole, its metrics against hand-computed
//! values, its degraded searches, its intervals, its reports, its runs and its
//! comparisons.

mod compare;
mod degraded;
mod documents;
mod failures;
mod groups;
mod intervals;
mod metrics;
mod ranking;
mod report;
mod run;
mod support;
mod v2;
mod v2_comparison;
mod v2_scoring;
mod v2_statistics;
