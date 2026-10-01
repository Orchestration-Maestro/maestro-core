//! Checking suites against their corpus: every name a suite of a directory
//! gives is exactly one section of its document, or, with an empty heading
//! path, a document without sections, as the evaluation runner resolves it;
//! and every unanswerable question, which names nothing as the suite's
//! contract holds, is probed for leads: the documents of the manifest that
//! hold all its identifier-like terms. The check runs here on the public
//! synthetic collection and on scratch suites, and on any suites and corpus
//! by hand (`local_suites.rs`).
#![cfg(test)]

mod check;
mod local_suites;
mod reported_problems;
mod scratch;
mod synthetic_suite;
mod unanswerable_leads;
