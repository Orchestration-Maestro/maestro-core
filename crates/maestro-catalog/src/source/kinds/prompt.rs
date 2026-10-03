//! Native prompts with declared input/output contracts and a filled inert body.

use super::contract;
use crate::source::{
    descriptor::{Format, KindDescriptor, Layout, MetadataPlace, Scope},
    rules::KindRules,
    types::{Known, Maturity, Problems, Resource},
};

/// Prompt placement uses a native Markdown file and common metadata sidecar.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "prompt".to_owned(),
        version: 1,
        directory: "prompts".to_owned(),
        scopes: vec![Scope::Common, Scope::Core, Scope::Team, Scope::Language],
        layout: Layout::Files {
            suffix: ".prompt.md".to_owned(),
            folders: vec![String::new()],
        },
        format: Format::Markdown,
        metadata: MetadataPlace::Sidecar {
            suffix: ".maestro.toml".to_owned(),
        },
        name_field: None,
        fields: contract::fields(),
        body: true,
        requires: vec!["contract".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: Some("prompt-contracts".to_owned()),
    }
}

/// Offline prompt validation, never template expansion or execution.
#[derive(Debug)]
pub(super) struct PromptRules;

impl KindRules for PromptRules {
    fn check_resource(
        &self,
        resource: &Resource,
        body: Option<&str>,
        _: Known<'_>,
        problems: &mut Problems,
    ) {
        contract::references(resource, problems);
        if let Some(body) = body
            && (body.contains("{{") || body.contains("${"))
        {
            problems.push((
                "body".to_owned(),
                "unfilled template; replace {{...}} and ${...} placeholders with authored text"
                    .to_owned(),
            ));
        }
    }
}
