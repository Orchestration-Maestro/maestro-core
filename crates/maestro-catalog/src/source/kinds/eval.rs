//! Eval case declarations own one exact inert JSON asset, never a runner.

use super::contract;
use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace, Scope},
    placements::safe,
    rules::KindRules,
    types::{Known, Maturity, Problems, Resource, Value},
};

use std::path::Path;

/// Minimal input/output contract bindings and a cases asset under the eval owner.
pub(super) fn descriptor() -> KindDescriptor {
    let mut fields = contract::fields();
    fields.push(Field::required("cases", FieldType::Text));
    KindDescriptor {
        kind: "eval-case".to_owned(),
        version: 1,
        directory: "evals".to_owned(),
        scopes: vec![Scope::Common, Scope::Core, Scope::Team, Scope::Language],
        layout: Layout::Files {
            suffix: ".toml".to_owned(),
            folders: vec![String::new()],
        },
        format: Format::Toml,
        metadata: MetadataPlace::Table {
            key: "metadata".to_owned(),
        },
        name_field: None,
        fields,
        body: false,
        requires: vec!["contract".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: Some("eval-contracts".to_owned()),
    }
}

/// Case data is inventoried under the bounded source snapshot, not evaluated.
#[derive(Debug)]
pub(super) struct EvalRules;

impl KindRules for EvalRules {
    fn check_resource(
        &self,
        resource: &Resource,
        _: Option<&str>,
        _: Known<'_>,
        problems: &mut Problems,
    ) {
        contract::references(resource, problems);
    }

    fn assets(&self, resource: &Resource) -> Result<Vec<String>, String> {
        let cases = resource
            .fields
            .get("cases")
            .and_then(Value::text)
            .ok_or("cases needs a JSON asset path")?;
        if !safe(cases, false) {
            return Err(format!(
                "cases needs an exact owner-relative asset path: {cases:?}"
            ));
        }
        if !Path::new(cases)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            return Err("cases must name an exact JSON asset".to_owned());
        }
        let parent = resource
            .path
            .rsplit_once('/')
            .map_or("", |(parent, _)| parent);
        Ok(vec![format!("{parent}/{cases}")])
    }
}
