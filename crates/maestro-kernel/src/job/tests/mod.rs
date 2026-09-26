//! Tests of jobs: their keys and attempts, their leases and states, their
//! progress in the journal, and a job a second process resumes, with the same
//! outcomes on Linux, macOS and Windows.

mod child;
mod errors;
mod leases;
mod progress;
mod resume;
mod states;
mod submit;
mod support;
