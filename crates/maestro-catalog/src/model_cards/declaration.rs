//! A model-card declaration uses catalog metadata plus the kernel's full v2 identity.

use maestro_kernel::gateway::CardIdentity;
use serde::Deserialize;
use std::collections::BTreeMap;

/// Strict TOML declaration envelope for a model card.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
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
    /// Common catalog resource metadata.
    pub(super) metadata: BTreeMap<String, toml::Value>,
    /// The kernel-owned identity, retained without a catalog copy of its schema.
    pub(super) identity: CardIdentity,
}
