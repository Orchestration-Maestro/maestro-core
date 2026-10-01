//! Discovery records for scoped descriptor discovery.

use super::{
    descriptor::{KindDescriptor, Layout, Scope},
    placements::{directories, fits},
    types::{Diagnostic, Refusal},
};

/// One discovered resource and its inert inventory.
#[derive(Debug)]
pub(super) struct Unit {
    /// Its registered kind.
    pub(super) kind: String,
    /// Its area namespace, absent only for area roots and presets.
    pub(super) namespace: Option<String>,
    /// Its local name.
    pub(super) name: String,
    /// Its primary file.
    pub(super) path: String,
    /// Its metadata sidecar, if any.
    pub(super) sidecar: Option<String>,
    /// Its exact inventoried inert files.
    pub(super) data: Vec<String>,
}

impl Unit {
    /// The resource `name` of `descriptor` at `path`.
    pub(super) fn new(descriptor: &KindDescriptor, name: &str, path: String) -> Self {
        Self {
            kind: descriptor.kind.clone(),
            namespace: namespace(descriptor, &path),
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

/// Derive identity only from a registered placement, never from a basename alias.
fn namespace(descriptor: &KindDescriptor, path: &str) -> Option<String> {
    if matches!(descriptor.layout, Layout::Area { .. }) {
        return None;
    }
    descriptor
        .scopes
        .iter()
        .zip(directories(descriptor))
        .find_map(|(scope, directory)| {
            let count = directory.split('/').filter(|part| !part.is_empty()).count();
            let boundary = path.split('/').take(count).collect::<Vec<_>>().join("/");
            if !fits(&directory, &boundary) {
                return None;
            }
            match scope {
                Scope::Root if descriptor.kind == "preset" => None,
                Scope::Root | Scope::Common => Some("common".to_owned()),
                _ => path
                    .split('/')
                    .nth(scope.prefix().split('/').count() - 1)
                    .map(str::to_owned),
            }
        })
}
