//! Clock and stage signals used only by the workspace's tests.
//! Fixtures are built before entering the held clock, so their setup never
//! spends the operation's deadline. This crate has no production dependency
//! on any Maestro crate.

mod stage_end;
mod stopped;

pub use stage_end::StageEnd;
pub use stopped::on_stopped_clock;
