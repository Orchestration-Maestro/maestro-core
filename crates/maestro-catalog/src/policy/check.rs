//! Cedar schema validation and authorization, fail-closed on every diagnostic.

use super::schema::{Check, Decision, HostFacts, Operation, TrustedFacts, bound_json, read};
use crate::{
    limits::Limits,
    source::{Directory, EntryKind, SourceTree as _},
};
use cedar_policy::{
    Authorizer, Context, Decision as CedarDecision, Entities, EntityUid, ParseErrors, PolicySet,
    Request, Schema, ValidationMode, Validator,
};
use serde_json::{Value, json};
use std::{path::Path, str::FromStr as _};

/// Replaceable effect-free evaluator. This port cannot execute an operation.
pub trait PolicyChecker {
    /// Checks normalized operation data against independently supplied host facts.
    fn check(&self, operation: &Operation, host: &dyn HostFacts) -> Check;
}

/// Real Cedar adapter, constructed only after strict schema/policy validation.
#[derive(Debug)]
pub struct Cedar {
    /// The validated schema for every request.
    schema: Schema,
    /// The policy set, evaluated together so forbids override permits.
    policies: PolicySet,
}

impl Cedar {
    /// Parses the real schema and policies and rejects every validation diagnostic.
    ///
    /// # Errors
    /// Parse errors, strict type errors and validation warnings.
    pub fn new(schema: &str, policies: &str) -> Result<Self, String> {
        let schema = Schema::from_json_str(schema).map_err(|error| error.to_string())?;
        let policies = PolicySet::from_str(policies).map_err(|error| parse_errors(&error))?;
        let validation = Validator::new(schema.clone()).validate(&policies, ValidationMode::Strict);
        let diagnostics: Vec<String> = validation
            .validation_errors()
            .map(ToString::to_string)
            .chain(validation.validation_warnings().map(ToString::to_string))
            .collect();
        if !diagnostics.is_empty() {
            return Err(diagnostics.join("\n"));
        }
        Ok(Self { schema, policies })
    }

    /// Builds a schema-validated request without interpreting submitted authority.
    fn request(
        &self,
        operation: &Operation,
        facts: Option<&TrustedFacts>,
    ) -> Result<Request, String> {
        if operation.action.trim().is_empty() || operation.target.trim().is_empty() {
            return Err("empty operation action or target".into());
        }
        if let Some(facts) = facts
            && (facts.actor.trim().is_empty()
                || facts.operation != operation.action
                || facts.target != operation.target)
        {
            return Err("trusted host facts are not bound to this actor/operation/target".into());
        }
        let actor = facts.map_or("untrusted", |facts| facts.actor.as_str());
        let principal = uid("Actor", actor)?;
        let action = uid("Action", &operation.action)?;
        let resource = uid("Target", &operation.target)?;
        let context = Context::from_json_value(context(facts), Some((&self.schema, &action)))
            .map_err(|error| error.to_string())?;
        Request::new(principal, action, resource, context, Some(&self.schema))
            .map_err(|error| error.to_string())
    }
}

impl PolicyChecker for Cedar {
    fn check(&self, operation: &Operation, host: &dyn HostFacts) -> Check {
        let facts = host.facts(operation);
        let request = match self.request(operation, facts.as_ref()) {
            Ok(request) => request,
            Err(message) => return Check::denied(message),
        };
        let response =
            Authorizer::new().is_authorized(&request, &self.policies, &Entities::empty());
        let diagnostics = response.diagnostics();
        let mut errors: Vec<String> = diagnostics.errors().map(ToString::to_string).collect();
        let mut policies: Vec<String> = diagnostics.reason().map(ToString::to_string).collect();
        policies.sort();
        let decision = if !errors.is_empty() {
            Decision::Deny
        } else if response.decision() == CedarDecision::Allow {
            Decision::Allow
        } else if facts.is_some()
            && !policies.is_empty()
            && diagnostics.reason().all(|id| {
                self.policies
                    .policy(id)
                    .is_some_and(|policy| policy.annotation("approval") == Some("required"))
            })
        {
            Decision::ApprovalNeeded
        } else {
            Decision::Deny
        };
        if decision == Decision::Deny && facts.is_none() {
            errors.push("needs trusted host facts (C20)".into());
        }
        Check {
            schema: "maestro-cli/policy-check/1",
            decision,
            policies,
            diagnostics: errors,
        }
    }
}

/// Quotes a normalized identifier as data, never Cedar expression text.
fn uid(kind: &str, identifier: &str) -> Result<EntityUid, String> {
    let quoted = serde_json::to_string(identifier).map_err(|error| error.to_string())?;
    format!("{kind}::{quoted}")
        .parse()
        .map_err(|error| parse_errors(&error))
}

/// Cedar's Display prints only the first parse error; preserve every one instead.
fn parse_errors(error: &ParseErrors) -> String {
    error
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Context contains only fixed absence information or host-supplied facts.
fn context(facts: Option<&TrustedFacts>) -> Value {
    match facts {
        None => json!({"trusted": false}),
        Some(facts) => json!({
            "trusted": true,
            "approved": facts.approved,
            "protected": facts.protected,
            "egress": facts.egress,
            "tool": facts.mcp_tool,
        }),
    }
}

/// Loads a fixture policy set or the core authoring policies, bounded by shared limits.
/// This is authoring input, not installed policy authority or execution admission.
///
/// # Errors
/// Missing, linked, oversized, invalid schema or invalid policies.
pub fn load(root: &Path, limits: &Limits) -> Result<Cedar, String> {
    let fixture = root.join("schema.json").exists();
    let directory = if fixture { "" } else { "core/policies" };
    let schema_path = if fixture {
        "schema.json"
    } else {
        "core/policies/schema.cedarschema.json"
    };
    let schema = read(root, Path::new(schema_path), limits)?;
    bound_json(&schema, limits)?;
    let entries = Directory::new(root)
        .list(directory)
        .map_err(|error| error.to_string())?;
    let mut policies = String::new();
    let mut count = 0;
    for entry in entries.into_iter().filter(|entry| {
        Path::new(&entry.name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("cedar"))
    }) {
        if entry.kind != EntryKind::File {
            return Err(format!("unsupported policy file {}", entry.name));
        }
        count += 1;
        if count > limits.catalog_resources {
            return Err("too many Cedar policy files".into());
        }
        let path = Path::new(directory).join(entry.name);
        let text = read(root, &path, limits)?;
        if policies.len() as u64 + text.len() as u64 + 1 > limits.source_file_bytes {
            return Err("combined Cedar policies exceed source byte limit".into());
        }
        policies.push_str(&text);
        policies.push('\n');
    }
    Cedar::new(&schema, &policies)
}
