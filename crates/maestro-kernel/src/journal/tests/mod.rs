//! Tests of the journal: its events and their streams, its cursors, and what
//! a crash leaves of them, with the same outcomes on Linux, macOS and Windows;
//! the envelope events leave the kernel in, and the committed schemas of the
//! public events.

mod append_only;
mod breaking;
mod child;
mod compatibility;
mod concurrency;
mod crash;
mod cursors;
mod envelope;
mod events;
mod regeneration;
mod schemas;
mod subset;
mod support;
mod validation;
