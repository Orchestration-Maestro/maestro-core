//! The quality gate (T020): what it records of each revision, what a rerun
//! and a ledger change, what it reports and why it stops, and the public
//! synthetic collection gated end to end.
#![cfg(test)]

mod gate_report;
mod kept_dispositions;
mod latest_revision;
mod recorded_dispositions;
mod support;
mod synthetic_corpus;
