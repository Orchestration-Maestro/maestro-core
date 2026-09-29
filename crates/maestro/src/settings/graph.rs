//! The configured local graph engine.

use super::session::Session;
use crate::failure::Failure;

/// The graph engine selected by `graph.engine`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphEngine {
    /// Graph reads are disabled and make no engine calls.
    None,
    /// The in-process `LadybugDB` engine.
    Lbug,
}

impl GraphEngine {
    /// The setting that selects it.
    pub(crate) const KEY: &str = "graph.engine";

    /// Reads the selected engine from the resolved settings.
    ///
    /// # Errors
    ///
    /// [`Failure::Failed`] when the registry lacks the setting or it holds
    /// another value, which the registry's choices rule out.
    pub(crate) fn from_session(session: &Session) -> Result<Self, Failure> {
        match session.resolved().text(Self::KEY) {
            Some("none") => Ok(Self::None),
            Some("lbug") => Ok(Self::Lbug),
            _ => Err(Failure::failed("graph.engine is missing or not a choice")),
        }
    }
}
