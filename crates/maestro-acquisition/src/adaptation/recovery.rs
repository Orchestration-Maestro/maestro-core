//! Small strict transaction markers, verified against the retained old pointer.
use super::{
    manifest::{Activation, WriteError},
    storage,
};
use crate::{Ref, Refusal, policy::manifest::AcquisitionManifest};
use maestro_kernel::{
    acquisition::{Progress, Reason, Receipts, Status},
    artifact::Digest,
};
use maestro_knowledge::strict_json::object;
use serde::{Deserialize, Serialize};

/// Closed transaction kinds; an activation cannot omit its required event.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum Recovery {
    /// One proposal added without changing the effective pointer.
    Proposal {
        /// Exact old pointer bytes, retained separately within the wire size limit.
        old: Digest,
        /// Exact committed pointer bytes.
        new: Digest,
        /// Unchanged processing identity.
        #[serde(deserialize_with = "object")]
        active: Ref,
    },
    /// Exactly one activation appended, with mandatory post-commit delivery.
    Activation {
        /// Exact old pointer bytes.
        old: Digest,
        /// Exact committed pointer bytes.
        new: Digest,
        /// Previously effective identity.
        #[serde(deserialize_with = "object")]
        previous_active: Ref,
        /// Content-free progress for the newly effective activation.
        event: Progress,
    },
}
impl Recovery {
    /// Construct a marker from the exact two typed pointers, before exposure.
    pub(super) fn new(old: Digest, new: Digest, active: Ref, event: Option<Progress>) -> Self {
        match event {
            Some(event) => Self::Activation {
                old,
                new,
                previous_active: active,
                event,
            },
            None => Self::Proposal { old, new, active },
        }
    }
    /// Both byte identities are independent of JSON's map implementation.
    pub(super) fn digests(&self) -> (&Digest, &Digest) {
        match self {
            Self::Proposal { old, new, .. } | Self::Activation { old, new, .. } => (old, new),
        }
    }
    /// Prove the declared kind from exact list and pointer transitions.
    pub(super) fn verify(
        &self,
        old: &AcquisitionManifest,
        new: &AcquisitionManifest,
        receipts: &dyn Receipts,
        principal: &str,
    ) -> Result<Option<Progress>, WriteError> {
        if old.resource != new.resource
            || old.baseline != new.baseline
            || old.baseline_kind != new.baseline_kind
        {
            return Err(Refusal::Invalid.into());
        }
        match self {
            Self::Proposal { active, .. } => {
                if &new.active != active
                    || new.active != old.active
                    || new.activations != old.activations
                    || appended(&old.proposals, &new.proposals).is_none()
                {
                    return Err(Refusal::Invalid.into());
                }
                Ok(None)
            }
            Self::Activation {
                previous_active,
                event,
                ..
            } => {
                if event.receipt.to_string() != new.active.id
                    || event.status != Status::Complete
                    || event.reason != Reason::None
                    || appended(&old.activations, &new.activations) != Some(&new.active)
                    || &old.active != previous_active
                {
                    return Err(Refusal::Invalid.into());
                }
                let activation: Activation = storage::artifact(receipts, principal, &new.active)?;
                if activation.previous != *previous_active {
                    return Err(Refusal::Invalid.into());
                }
                if new.proposals != old.proposals
                    && appended(&old.proposals, &new.proposals) != Some(&activation.proposal)
                {
                    return Err(Refusal::Invalid.into());
                }
                Ok(Some(*event))
            }
        }
    }
}

/// An exact append retains every earlier identity and adds precisely one.
fn appended<'a>(old: &[Ref], new: &'a [Ref]) -> Option<&'a Ref> {
    let (last, prefix) = new.split_last()?;
    if prefix != old {
        return None;
    }
    Some(last)
}
