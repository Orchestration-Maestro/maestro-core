//! A model-card declaration uses catalog metadata plus the kernel's full v2 identity.

use crate::source::{Maturity, Resource, Value};
use maestro_kernel::gateway::CardIdentity;

/// A model-card declaration built only from a resource accepted by the catalog checker.
#[derive(Debug)]
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "C16h is the first production caller for admitted installed declarations"
    )
)]
pub(super) struct Declaration {
    /// Version of the declaration's model-card shape.
    pub(super) version: String,
    /// The checked catalog lifecycle stage.
    pub(super) maturity: Maturity,
    /// The kernel-owned identity, retained without a catalog copy of its schema.
    pub(super) identity: CardIdentity,
}

impl Declaration {
    /// Builds the adapter input from a catalog-checked resource.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "C16h is the first production caller and receives checked resources"
        )
    )]
    pub(super) fn from_resource(resource: &Resource) -> Result<Self, String> {
        let Some(Value::Text(version)) = resource.fields.get("version") else {
            return Err("missing checked model-card version".to_owned());
        };
        Ok(Self {
            version: version.clone(),
            maturity: resource.metadata.maturity,
            identity: resource
                .fields
                .get("identity")
                .ok_or_else(|| "missing checked model-card identity".to_owned())?
                .decode()?,
        })
    }
}
