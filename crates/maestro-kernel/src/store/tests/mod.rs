//! Tests of the kernel database: its migrations, its connections, the
//! artifacts it records and their garbage collection, with the same outcomes
//! on Linux, macOS and Windows.

mod artifacts;
mod checks;
mod connections;
mod garbage;
mod migrations;
mod support;
