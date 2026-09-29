//! Shared limits and kernel-open seam for the MCP server.

use crate::{failure::Failure, kernel::Kernel};
use std::{sync::Arc, time::Duration};

/// Schema identifier embedded in JSON text for MCP tool errors.
pub(super) const TOOL_ERROR_SCHEMA: &str = "maestro-mcp-error/1";
/// Maximum number of simultaneous local knowledge operations.
pub(super) const WORKER_LIMIT: usize = 4;
/// Per-call deadline, above the longest search (30 s) and under the 60 s
/// tool timeout common to MCP clients; blocking workers retain their permit
/// until the operation ends.
pub(super) const CALL_DEADLINE: Duration = Duration::from_secs(40);
/// Ask's deadline: the longest search and one cold chat call (20 s), under
/// the 60 s tool timeout common to MCP clients. A cold ask ends well within
/// it, since a cold search takes about 8 s, not its 30 s cap.
pub(super) const ASK_CALL_DEADLINE: Duration = Duration::from_secs(55);
/// Local kernel opener kept behind a private test seam.
pub(super) type KernelOpener = Arc<dyn Fn() -> Result<Kernel, Failure> + Send + Sync>;
