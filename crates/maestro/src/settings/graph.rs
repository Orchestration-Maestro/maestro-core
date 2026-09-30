//! The configured local graph engine.

use super::session::Session;
use crate::failure::Failure;
use std::collections::BTreeSet;

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
        Self::read(session).map(|(engine, _)| engine)
    }

    /// Reads the selected engine and reports the setting key it read.
    pub(crate) fn read(session: &Session) -> Result<(Self, BTreeSet<String>), Failure> {
        let engine = match session.resolved().text(Self::KEY) {
            Some("none") => Self::None,
            Some("lbug") => Self::Lbug,
            _ => return Err(Failure::failed("graph.engine is missing or not a choice")),
        };
        Ok((engine, BTreeSet::from([Self::KEY.to_owned()])))
    }
}
