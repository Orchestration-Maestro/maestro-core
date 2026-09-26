//! Tests of the evaluation runner: how it ranks and judges each question,
//! the documents a question expects whole, its metrics against hand-computed
//! values, its degraded searches, its intervals, its reports, its runs and its
//! comparisons.

mod compare;
mod degraded;
mod documents;
mod failures;
mod intervals;
mod metrics;
mod ranking;
mod report;
mod run;
mod support;
