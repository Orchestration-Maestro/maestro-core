//! Process-local acquisition lifecycle accounting.
pub mod resources;

pub mod full;
pub mod incremental;

pub mod resume;
pub mod schedule;

pub(crate) mod schedule_stop;
