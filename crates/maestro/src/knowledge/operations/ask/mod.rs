//! The shared knowledge ask operation.

#[cfg(test)]
use self::run::{answer_failure, registered_answerer};
/// Executes the ask operation for the CLI and MCP.
pub(crate) mod run;
#[cfg(test)]
pub(crate) mod tests;
