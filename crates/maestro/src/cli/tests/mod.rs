//! The command line's unit tests: the lease a foreground job holds, how an
//! import's steps and end become its job's, and how an import supersedes
//! the job that holds its resource; the contract tests of the built binary
//! are the crate's integration tests.

mod import_endings;
mod lease_heartbeats;
mod supersessions;
mod support;
