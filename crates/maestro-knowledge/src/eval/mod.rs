//! The evaluation runner (plan D13; FR-S1-009, SC-S1-008): every retrieval
//! choice of S1 is decided by measuring it on a suite, `maestro-suite/1`
//! ([`crate::suite`]).
//!
//! [`run()`] is a library: it takes what it evaluates ([`Header`]), a suite, a
//! lookup of the canonical documents of the evaluated generation by
//! `source_ref`, and a retrieval, any function from a question to an evidence
//! bundle, `maestro-evidence/1`. It first resolves every section the suite
//! expects to its ID in that generation, looking each document up once, and
//! refuses a name that gives no one section, and a question that names one
//! section twice, before any retrieval. Then it retrieves each question in
//! the suite's order, times the retrieval, refuses a bundle of another
//! collection or generation, and judges it.
//!
//! A bundle lists its passages in reading order, so the runner ranks them
//! itself: by the reranker's score in the bundle's trace, highest first, then
//! the passages the trace does not score, or scores with a number that is not
//! finite; passages of equal score, and unscored ones, by passage number,
//! lowest first, which evidence assembly (T032) gives in rank order.
//! Relevance is binary: a passage is relevant when its section is one the
//! question expects, and a section found twice counts once, at its best rank.
//! The metrics ([`Metrics`]):
//!
//! - Recall@5 and Recall@10: the share of answerable questions with an
//!   expected section in the top 5, or 10 (02 §10);
//! - MRR@10: the mean reciprocal rank of the first expected section, 0 below
//!   the top 10;
//! - nDCG@10: each expected section gains 1 at its best rank within the top
//!   10, discounted by log2(rank + 1), over the gain of the ideal ranking,
//!   which holds every expected section, retrieved or not;
//! - no-answer accuracy: the share of unanswerable questions whose bundle
//!   holds no passage (02 §10), and apart from it the false abstentions, the
//!   share of answerable questions whose bundle holds none. The decision to
//!   return nothing belongs to evidence assembly (T032) and `ask` (T035); the
//!   runner only measures it;
//! - latency p50 and p95: the nearest-rank percentiles of the retrievals'
//!   times, in microseconds, over the searches that were not degraded.
//!
//! A search is degraded when a route or the reranker could not run, as its
//! bundle says. It counts in every retrieval metric, but its time is left out
//! of the latency percentiles, and the report counts degraded searches apart
//! (SC-S1-004). A search that waited for a model to load says so nowhere in
//! its bundle, so the runner cannot tell it apart.
//!
//! Each metric covers its questions only, and is absent from a run that has
//! none of them. Its interval is the 2.5th and 97.5th percentiles of its
//! values over 2,000 bootstrap resamples, each drawing as many answerable and
//! as many unanswerable questions as the run holds, with replacement, by a
//! `SplitMix64` generator the report's seed starts; a resample without a
//! search that was not degraded gives no latency. The same seed gives the
//! same intervals, bit for bit, on every platform: the metrics use the basic
//! arithmetic IEEE 754 fixes and nothing of the platform's math library, since
//! nDCG's ten discounts are written out.
//!
//! An answerable question fails at a cut-off, 5 or 10, when no expected
//! section ranks within it: it is `not_retrieved` when no passage holds an
//! expected section, `misranked` when one does, below the cut-off
//! ([`FailureClass`]). Each failure names the routes that ran, the reranker
//! aside, and whose trace found no expected section, and the routes that could
//! not run, the reranker among them, with their reasons. A wrong answer is
//! the class of `ask` (T035). An unanswerable question has no failure class:
//! a bundle that holds passages counts against the no-answer accuracy.
//! Command exactness needs expected answers, which `maestro-suite/1` does not
//! carry, so it is not measured here.
//!
//! A run's [`Report`], `maestro-eval-report/1`, names the suite, the
//! collection, the generation, its profiles and the seed, with each question's
//! result, the number of degraded searches and the metrics. It is strict JSON:
//! its reader refuses an unknown key, a key given twice, an object written as
//! an array and a metric written `null`. The kernel stores it as an artifact
//! and indexes it ([`maestro_kernel::eval`]).
//!
//! [`compare()`] pairs two runs of one suite by question id and gives, for
//! each metric, the candidate's value minus the baseline's, with the interval
//! of 2,000 paired resamples drawn the same way under the seed it is given.

mod bootstrap;
mod compare;
mod error;
mod judge;
mod metric;
mod report;
mod run;
#[cfg(test)]
mod tests;

pub use compare::{Comparison, compare};
pub use error::{CompareError, RunError};
pub use report::{
    Estimate, Expected, Failure, FailureClass, Header, Metrics, QuestionResult, Report, Schema,
};
pub use run::run;
