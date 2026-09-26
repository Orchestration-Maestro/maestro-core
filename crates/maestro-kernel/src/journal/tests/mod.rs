//! Tests of the journal: its events and their streams, its cursors, and what
//! a crash leaves of them, with the same outcomes on Linux, macOS and Windows.

mod append_only;
mod child;
mod concurrency;
mod crash;
mod cursors;
mod events;
mod support;
