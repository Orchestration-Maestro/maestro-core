//! Private-content fingerprinting and outgoing Git-object scanning.

/// Authenticated SQLite bank access and full validation.
mod bank;
/// SQLite bank construction from private source snapshots.
mod bank_build;
/// Argument parsing and output for the privacy command.
mod cli;
/// Scanning the outgoing objects against the fingerprint bank.
mod git;
/// Git plumbing for walking explicit outgoing object sets.
mod git_objects;
/// Prepared SQL statements reused during one private scan.
mod lookup;
/// HMAC-SHA-256 primitive shared by fingerprints and bank integrity.
mod mac;
/// Unicode normalization and keyed fingerprint generation.
mod normalize;

pub use cli::run;
