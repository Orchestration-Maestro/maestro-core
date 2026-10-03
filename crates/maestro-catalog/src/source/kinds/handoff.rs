//! Inert handoffs with typed sender/recipient, contracts and fixed body sections.

use super::{agent::section_problems, contract};
use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
    rules::KindRules,
    types::{Known, Maturity, Problems, Resource},
};

/// Framework/team handoff declarations and their exact metadata pairs.
pub(super) fn descriptor() -> KindDescriptor {
    let mut fields = contract::fields();
    fields.extend([
        Field::required("sender", FieldType::Text),
        Field::required("recipient", FieldType::Text),
    ]);
    KindDescriptor {
        kind: "handoff".to_owned(),
        version: 1,
        directory: "handoffs".to_owned(),
        scopes: vec![Scope::Core, Scope::Team],
        layout: Layout::Files {
            suffix: ".handoff.md".to_owned(),
            folders: vec![String::new()],
        },
        format: Format::Markdown,
        metadata: MetadataPlace::Sidecar {
            suffix: ".maestro.toml".to_owned(),
        },
        name_field: None,
        fields,
        body: true,
        requires: vec!["agent".to_owned(), "contract".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: Some("handoff-contracts".to_owned()),
    }
}

/// Structural declarations only, not runtime readiness or independent review.
#[derive(Debug)]
pub(super) struct HandoffRules;

impl KindRules for HandoffRules {
    fn check_resource(
        &self,
        resource: &Resource,
        body: Option<&str>,
        _: Known<'_>,
        problems: &mut Problems,
    ) {
        contract::references(resource, problems);
        for key in ["sender", "recipient"] {
            contract::reference(resource, key, "agent", problems);
        }
        if let Some(body) = body {
            section_problems(
                body,
                &["Inputs", "Context", "Deliverables", "Acceptance"],
                problems,
            );
        }
    }
}
