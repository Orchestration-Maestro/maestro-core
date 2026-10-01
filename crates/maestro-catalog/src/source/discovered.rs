//! Discovery records shared by the legacy and scoped descriptor walkers.

use super::{
    descriptor::KindDescriptor,
    types::{Diagnostic, Refusal},
};

/// One discovered resource and its inert inventory.
#[derive(Debug)]
pub(super) struct Unit {
    /// Its registered kind.
    pub(super) kind: String,
    /// Its local name; qualification is supplied by C32.
    pub(super) name: String,
    /// Its primary file.
    pub(super) path: String,
    /// Its metadata sidecar, if any.
    pub(super) sidecar: Option<String>,
    /// Its inert data folders (legacy) or exact inventoried files (v4).
    pub(super) data: Vec<String>,
}

impl Unit {
    /// The resource `name` of `descriptor` at `path`.
    pub(super) fn new(descriptor: &KindDescriptor, name: &str, path: String) -> Self {
        Self {
            kind: descriptor.kind.clone(),
            name: name.to_owned(),
            path,
            sidecar: None,
            data: Vec::new(),
        }
    }
}

/// Resources and layout diagnostics, before any content parsing.
#[derive(Debug, Default)]
pub(super) struct Found {
    /// Discovered resources.
    pub(super) units: Vec<Unit>,
    /// Discovery refusals.
    pub(super) diagnostics: Vec<Diagnostic>,
}

/// One whole-catalog bound refusal.
pub(super) fn refusal(message: String) -> Refusal {
    Refusal {
        diagnostics: vec![Diagnostic::new("", "", message)],
    }
}
