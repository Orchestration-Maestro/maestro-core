//! Shared limits and kernel-open seam for the MCP server.

use crate::{failure::Failure, kernel::Kernel};
use std::{sync::Arc, time::Duration};

/// Schema identifier embedded in JSON text for MCP tool errors.
pub(super) const TOOL_ERROR_SCHEMA: &str = "maestro-mcp-error/1";
/// Maximum number of simultaneous local knowledge operations.
pub(super) const WORKER_LIMIT: usize = 4;
/// Per-call deadline; blocking workers retain their permit until the operation ends.
pub(super) const CALL_DEADLINE: Duration = Duration::from_secs(15);
/// Ask may use search and two individually bounded ten-second chat calls.
pub(super) const ASK_CALL_DEADLINE: Duration = Duration::from_secs(35);
/// Local kernel opener kept behind a private test seam.
pub(super) type KernelOpener = Arc<dyn Fn() -> Result<Kernel, Failure> + Send + Sync>;
