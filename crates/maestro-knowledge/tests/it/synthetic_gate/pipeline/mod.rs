//! The verified synthetic import, evaluation and reporting pipeline.

pub(in crate::synthetic_gate) mod baseline;
mod contract;
mod driver;
mod fixture;
mod readiness;
mod report;
mod runner;

pub(super) use contract::{Output, assert_repeated_run_identity};
pub(super) use report::{append_summary, summary};
pub(super) use runner::{run_fake, run_fake_with_dead_endpoint, run_real_qdrant_baseline};
