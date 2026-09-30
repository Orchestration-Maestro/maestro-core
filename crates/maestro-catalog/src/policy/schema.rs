//! Normalized data, separate host facts, and bounded authoring test inputs.

use crate::limits::Limits;
use maestro_filesystem::Directory;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, io::Read, path::Path};

/// Data describing the operation, never identity or approval.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    /// Schema-declared action, not executable command text.
    pub action: String,
    /// Normalized target identifier.
    pub target: String,
}

impl Operation {
    /// Parses bounded strict operation data. Extra authority fields refuse.
    ///
    /// # Errors
    /// Oversized, malformed or unknown fields.
    pub fn parse(text: &str, limits: &Limits) -> Result<Self, String> {
        bound_json(text, limits)?;
        serde_json::from_str(text).map_err(|error| error.to_string())
    }
}

/// Facts supplied only by a trusted host adapter, bound to one operation.
/// Deserialization is for authoring test stimuli only, never check input.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedFacts {
    /// Host-authenticated actor.
    pub actor: String,
    /// Exact action these facts describe.
    pub operation: String,
    /// Exact normalized target these facts describe.
    pub target: String,
    /// Independently verified user approval, not permission by itself.
    pub approved: bool,
    /// Target is protected according to host normalization.
    pub protected: bool,
    /// Destination is eligible under the host's egress allowlist.
    pub egress: bool,
    /// Host-normalized MCP tool name, empty for a non-MCP operation.
    pub mcp_tool: String,
}

/// Supplies authority separately from operation text. There is no execution API.
pub trait HostFacts {
    /// Returns authenticated facts bound to this operation, or none.
    fn facts(&self, operation: &Operation) -> Option<TrustedFacts>;
}

/// C19's production host adapter: no host qualification until C20.
#[derive(Debug, Clone, Copy)]
pub struct NoFacts;

impl HostFacts for NoFacts {
    fn facts(&self, _operation: &Operation) -> Option<TrustedFacts> {
        None
    }
}

/// Check outcome; approval-needed never grants permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// Cedar allowed with no diagnostics.
    Allow,
    /// Default deny, explicit forbid, invalid data or a diagnostic.
    Deny,
    /// Only approval-labelled forbids matched; still refused.
    ApprovalNeeded,
}

/// Effect-free check result, with all Cedar diagnostics and matching policy IDs.
#[derive(Debug, Serialize)]
pub struct Check {
    /// Stable machine schema.
    pub schema: &'static str,
    /// The decision, never an execution receipt.
    pub decision: Decision,
    /// Matched Cedar policy IDs, sorted.
    pub policies: Vec<String>,
    /// Every validation/evaluation error; any error denies.
    pub diagnostics: Vec<String>,
}

impl Check {
    /// Denies malformed input or missing trusted facts.
    #[must_use]
    pub fn denied(message: String) -> Self {
        Self {
            schema: "maestro-cli/policy-check/1",
            decision: Decision::Deny,
            policies: Vec::new(),
            diagnostics: vec![message],
        }
    }
}

/// An authoring stimulus, not a production source of trusted facts.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    /// Unique nonempty case name.
    pub name: String,
    /// Submitted normalized operation.
    pub operation: Operation,
    /// Synthetic host facts, unavailable to `policy check`.
    pub facts: Option<TrustedFacts>,
    /// Required outcome.
    pub expected: Decision,
}

/// Reads at most the source limit plus one byte, for stdin adapters.
///
/// # Errors
/// Read errors, oversized input or non-UTF-8.
pub fn read_input(reader: impl Read, limits: &Limits) -> Result<String, String> {
    let mut bytes = Vec::new();
    reader
        .take(limits.source_file_bytes.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    let text = String::from_utf8(bytes).map_err(|error| error.to_string())?;
    bound(&text, limits)?;
    Ok(text)
}

/// Bounds text before either Cedar or serde allocates its parse tree.
fn bound(text: &str, limits: &Limits) -> Result<(), String> {
    if text.len() as u64 > limits.source_file_bytes {
        return Err(format!(
            "policy input larger than {} bytes",
            limits.source_file_bytes
        ));
    }
    Ok(())
}

/// Reads a bounded regular file without following links below the authoring root.
pub(super) fn read(root: &Path, relative: &Path, limits: &Limits) -> Result<String, String> {
    let parent = relative.parent().unwrap_or_else(|| Path::new(""));
    let name = relative
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("invalid policy filename")?;
    let directory = Directory::open(root, parent, false).map_err(|error| error.to_string())?;
    let bytes = directory
        .read_regular_bounded(name, limits.source_file_bytes)
        .map_err(|error| error.to_string())?;
    let text = String::from_utf8(bytes).map_err(|error| error.to_string())?;
    bound(&text, limits)?;
    Ok(text)
}

/// Loads authoring cases from the fixture directory or the core scenario location.
///
/// # Errors
/// Missing, empty, duplicate, malformed or oversized stimuli refuse.
pub fn test_cases(root: &Path, limits: &Limits) -> Result<Vec<Case>, String> {
    let relative = if root.join("schema.json").exists() {
        "cases.json"
    } else {
        "core/evals/scenarios/policy-neighbours.json"
    };
    let text = read(root, Path::new(relative), limits)?;
    bound_json(&text, limits)?;
    let cases: Vec<Case> = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    if cases.is_empty() {
        return Err("zero policy test cases discovered".into());
    }
    if cases.len() > limits.catalog_resources {
        return Err("too many policy test cases".into());
    }
    let mut names = BTreeSet::new();
    for case in &cases {
        if case.name.trim().is_empty() || !names.insert(&case.name) {
            return Err("empty or duplicate policy case name".into());
        }
    }
    Ok(cases)
}

/// Bounds JSON containers before any parser allocates a tree.
/// Quotes and escapes are skipped; the subsequent parser validates syntax.
pub(super) fn bound_json(text: &str, limits: &Limits) -> Result<(), String> {
    bound(text, limits)?;
    let mut depth: usize = 0;
    let mut quoted = false;
    let mut escaped = false;
    for byte in text.bytes() {
        if escaped {
            escaped = false;
        } else if quoted && byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            quoted = !quoted;
        } else if !quoted {
            match byte {
                b'{' | b'[' => {
                    depth += 1;
                }
                b'}' | b']' => depth = depth.saturating_sub(1),
                _ => {}
            }
        }
        if depth > limits.source_depth {
            return Err(format!(
                "policy JSON depth exceeds {} levels",
                limits.source_depth
            ));
        }
    }
    Ok(())
}
