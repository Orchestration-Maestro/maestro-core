//! Tests of jobs: their keys and attempts, their resources, their scopes,
//! their leases and states, the events of their changes and their progress in
//! the journal, their table, and a job a second process resumes, with the same
//! outcomes on Linux, macOS and Windows.

mod changes;
mod child;
mod errors;
mod leases;
mod progress;
mod resources;
mod resume;
mod scopes;
mod states;
mod submit;
mod support;
mod table;
