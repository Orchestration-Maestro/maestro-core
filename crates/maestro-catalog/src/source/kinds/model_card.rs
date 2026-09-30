//! `model-card`: a strict catalog declaration of the kernel's v2 identity.

use crate::source::{
    descriptor::{Field, FieldType, Format, KindDescriptor, Layout, MetadataPlace},
    rules::KindRules,
    types::{Known, Maturity, Problems, Resource},
};
use maestro_kernel::gateway::{CardIdentity, ModelCard};

/// The model-card kind.
pub(super) fn descriptor() -> KindDescriptor {
    KindDescriptor {
        kind: "model-card".to_owned(),
        version: 1,
        directory: "model-cards".to_owned(),
        layout: Layout::Files {
            suffix: ".toml".to_owned(),
            folders: vec![String::new()],
        },
        format: Format::Toml,
        metadata: MetadataPlace::Table {
            key: "metadata".to_owned(),
        },
        name_field: None,
        fields: vec![
            Field::required("version", FieldType::Text),
            Field::required(
                "identity",
                FieldType::Delegated {
                    validator: "model-card".to_owned(),
                },
            ),
        ],
        body: false,
        requires: vec!["*".to_owned()],
        lifecycle: Maturity::DECLARABLE.to_vec(),
        closure_root: false,
        required: None,
        hook: Some("model-card".to_owned()),
    }
}

/// Validates the exact nested identity through the kernel-owned type.
#[derive(Debug)]
pub(super) struct ModelCardRules;

impl KindRules for ModelCardRules {
    fn check_resource(
        &self,
        resource: &Resource,
        _body: Option<&str>,
        _known: Known<'_>,
        problems: &mut Problems,
    ) {
        match declaration_identity(resource) {
            Ok(identity) => {
                if let Err(error) = ModelCard::from_identity(&identity) {
                    problems.push(("identity".to_owned(), error.to_string()));
                }
            }
            Err(error) => problems.push(("identity".to_owned(), error)),
        }
    }
}

/// Decodes the complete subtree as the kernel's canonical v2 identity.
pub(super) fn declaration_identity(resource: &Resource) -> Result<CardIdentity, String> {
    resource
        .fields
        .get("identity")
        .ok_or_else(|| "missing".to_owned())?
        .decode()
}
