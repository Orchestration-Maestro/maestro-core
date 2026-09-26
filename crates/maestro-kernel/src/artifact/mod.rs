//! Content-addressed artifacts: immutable bytes stored, and read back, by their
//! SHA-256 digest (building block B3).
//!
//! An artifact lives at `sha256/<2 hex>/<2 hex>/<64 hex>` under the store's
//! root. A write goes to a temporary file in the same directory, is flushed,
//! renamed into place and the directory flushed, so a reader never sees half
//! an artifact; each directory the store creates is flushed into its parent
//! too. Every read is checked against the digest, and only a regular file is
//! read.
//!
//! The store behaves the same on Linux, macOS and Windows, with two
//! differences the platforms impose. The files and directories it creates
//! are its owner's only: `0600` and `0700` on Linux and macOS, while on
//! Windows they take the access list of their directory, the user's profile.
//! Windows cannot flush a directory through a handle the standard library
//! opens, so there each file is flushed before its rename and the directory
//! flush is skipped, as ADR-0018 records for the snapshot store.
//!
//! A file named `.tmp-<process>-<number>` is a write in progress, or one a
//! crash interrupted: no artifact refers to it.

mod digest;
mod store;
#[cfg(test)]
mod tests;

pub use digest::{Digest, InvalidDigest};
pub use store::{Error, Store};
