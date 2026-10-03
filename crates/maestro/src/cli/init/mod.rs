//! Init's shared flow and plain adapter; the catalog planner remains independent.
mod command;
pub(in crate::cli) mod flow;
mod menu;
pub(in crate::cli) mod plain;
pub(in crate::cli) mod terminal;
#[cfg(test)]
mod tests;

pub(super) use command::ApplyChoices;
pub(super) use menu::run::{Request, run};
