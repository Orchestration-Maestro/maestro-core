//! Resolution values and provenance shared by the single C17 resolver.

use maestro_settings::Value;
use std::collections::BTreeMap;

/// A chosen value with its source and values it overrode.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedValue {
    /// The effective typed value.
    pub(super) value: Value,
    /// The layer that supplied or constrained the value.
    pub(super) source: Layer,
    /// Every supplied value other than the effective candidate.
    pub(super) overridden: Vec<(Layer, Value)>,
}

/// Preference and constraint sources, plus accumulated values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// Explicit command-line value.
    Flag,
    /// Workspace configuration file.
    Workspace,
    /// User configuration file.
    User,
    /// Built-in descriptor default.
    Default,
    /// Admitted standard constraint.
    Standard,
    /// Accumulated additive value.
    Combined,
}

impl Layer {
    /// Stable display name for provenance.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Workspace => "workspace",
            Self::User => "user",
            Self::Default => "default",
            Self::Standard => "standard",
            Self::Combined => "combined",
        }
    }
}

/// A setting could not be resolved because it was locked or invalid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveDiagnostic {
    /// The setting key.
    pub key: String,
    /// Why the value was refused or ignored.
    pub message: String,
}

/// All effective values and diagnostics from a resolution pass.
#[derive(Debug, Clone, Default)]
pub struct ResolvedSettings {
    /// Every registry key's resolution result.
    pub(super) values: BTreeMap<String, Result<ResolvedValue, ResolveDiagnostic>>,
    /// Ignored requests that would widen a restrictive value.
    pub(super) diagnostics: Vec<ResolveDiagnostic>,
}

impl ResolvedValue {
    /// The effective typed value.
    #[must_use]
    pub fn value(&self) -> &Value {
        &self.value
    }

    /// The value's source layer.
    #[must_use]
    pub fn source(&self) -> &'static str {
        self.source.name()
    }

    /// Lower-priority or ignored layer values.
    #[must_use]
    pub fn overridden(&self) -> &[(Layer, Value)] {
        &self.overridden
    }
}

impl ResolvedSettings {
    /// The resolved value for `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&Result<ResolvedValue, ResolveDiagnostic>> {
        self.values.get(key)
    }

    /// The effective text value of `key`.
    #[must_use]
    pub fn text(&self, key: &str) -> Option<&str> {
        match self.get(key)?.as_ref().ok()?.value() {
            Value::Text(text) => Some(text),
            _ => None,
        }
    }

    /// The effective integer value of `key`.
    #[must_use]
    pub fn integer(&self, key: &str) -> Option<i64> {
        match self.get(key)?.as_ref().ok()?.value() {
            Value::Integer(integer) => Some(*integer),
            _ => None,
        }
    }

    /// Ignored widening requests; refusals are returned by `get`.
    #[must_use]
    pub fn diagnostics(&self) -> &[ResolveDiagnostic] {
        &self.diagnostics
    }
}

/// Constructs a keyed resolution diagnostic.
pub(super) fn diagnostic(key: &str, message: impl Into<String>) -> ResolveDiagnostic {
    ResolveDiagnostic {
        key: key.to_owned(),
        message: message.into(),
    }
}

/// Applies ordinary first-layer-wins precedence.
pub(super) fn first(default: Value, candidates: Vec<(Layer, Value)>) -> ResolvedValue {
    let mut candidates = candidates.into_iter();
    let (source, value) = candidates.next().unwrap_or((Layer::Default, default));
    ResolvedValue {
        value,
        source,
        overridden: candidates.collect(),
    }
}
