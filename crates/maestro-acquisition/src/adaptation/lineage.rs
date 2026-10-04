//! Validate only payloads that contribute to the effective configuration.
use super::{
    manifest::{Activation, Proposal, WriteError},
    storage,
};
use crate::{Ref, Refusal, policy::manifest::AcquisitionManifest};
use maestro_kernel::acquisition::Receipts;

/// Ordered pointer metadata implies ancestry without reading denied history.
pub(super) fn previous<'a>(
    manifest: &'a AcquisitionManifest,
    reference: &Ref,
) -> Result<&'a Ref, WriteError> {
    let index = manifest
        .activations
        .iter()
        .rposition(|value| value == reference)
        .ok_or(Refusal::Invalid)?;
    Ok(index
        .checked_sub(1)
        .and_then(|index| manifest.activations.get(index))
        .unwrap_or(&manifest.baseline))
}

/// Walk strictly earlier identities, following restores for rollback entries.
pub(super) fn validate(
    receipts: &dyn Receipts,
    principal: &str,
    manifest: &AcquisitionManifest,
    start: &Ref,
) -> Result<(), WriteError> {
    let mut reference = start;
    // Own payloads so the next reference remains valid across loop iterations.
    let mut next;
    while reference != &manifest.baseline {
        let activation: Activation = storage::artifact(receipts, principal, reference)?;
        let proposal: Proposal = storage::proposal(receipts, principal, &activation)?;
        bindings(manifest, reference, &activation, &proposal)?;
        next = activation.restores.unwrap_or(activation.previous);
        reference = &next;
    }
    Ok(())
}

/// Verify ancestry before following any restored pointer.
fn bindings(
    manifest: &AcquisitionManifest,
    reference: &Ref,
    activation: &Activation,
    proposal: &Proposal,
) -> Result<(), WriteError> {
    let implied = previous(manifest, reference)?;
    if activation.previous != *implied
        || proposal.expected_active != *implied
        || proposal.expected_baseline != manifest.baseline
    {
        return Err(Refusal::Invalid.into());
    }
    if let Some(restores) = &activation.restores {
        let earlier = manifest
            .activations
            .iter()
            .take_while(|value| *value != reference);
        if restores != &manifest.baseline && !earlier.into_iter().any(|value| value == restores) {
            return Err(Refusal::Invalid.into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Activation, Proposal, bindings, storage};
    use crate::{Ref, policy::manifest::AcquisitionManifest};

    #[test]
    fn n30_duplicate_active_entry_cannot_rebind_previous() {
        let mut manifest: AcquisitionManifest =
            storage::decode(include_bytes!("golden-manifest.json")).unwrap();
        let proposal: Proposal = storage::decode(include_bytes!("golden-proposal.json")).unwrap();
        let activation: Activation =
            storage::decode(include_bytes!("golden-activation.json")).unwrap();
        let reference = Ref {
            id: "active".into(),
            digest: manifest.baseline.digest.clone(),
        };
        manifest.activations = vec![reference.clone(), reference.clone()];
        assert!(bindings(&manifest, &reference, &activation, &proposal).is_err());
    }

    #[test]
    fn n30_lineage_bindings_are_checked_before_following() {
        let mut manifest: AcquisitionManifest =
            storage::decode(include_bytes!("golden-manifest.json")).unwrap();
        let proposal: Proposal = storage::decode(include_bytes!("golden-proposal.json")).unwrap();
        let activation: Activation =
            storage::decode(include_bytes!("golden-activation.json")).unwrap();
        let reference = Ref {
            id: "active".into(),
            digest: manifest.baseline.digest.clone(),
        };
        manifest.activations.push(reference.clone());
        for field in ["previous", "expected_active", "expected_baseline"] {
            let mut payload = activation.clone();
            let mut candidate = proposal.clone();
            match field {
                "previous" => payload.previous = reference.clone(),
                "expected_active" => candidate.expected_active = reference.clone(),
                _ => candidate.expected_baseline = reference.clone(),
            }
            assert!(
                bindings(&manifest, &reference, &payload, &candidate).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn n30_restores_must_name_baseline_or_earlier_activation() {
        let mut manifest: AcquisitionManifest =
            storage::decode(include_bytes!("golden-manifest.json")).unwrap();
        let proposal: Proposal = storage::decode(include_bytes!("golden-proposal.json")).unwrap();
        let reference = Ref {
            id: "active".into(),
            digest: manifest.baseline.digest.clone(),
        };
        let mut activation: Activation =
            storage::decode(include_bytes!("golden-activation.json")).unwrap();
        manifest.activations.push(reference.clone());
        assert!(bindings(&manifest, &reference, &activation, &proposal).is_ok());
        for target in [
            reference.clone(),
            Ref {
                id: "missing".into(),
                digest: reference.digest.clone(),
            },
        ] {
            activation.restores = Some(target);
            assert!(bindings(&manifest, &reference, &activation, &proposal).is_err());
        }
    }
    #[test]
    fn s6t_lineage_restores_exact_earlier_activation() {
        let mut manifest: AcquisitionManifest =
            storage::decode(include_bytes!("golden-manifest.json")).unwrap();
        let mut proposal: Proposal =
            storage::decode(include_bytes!("golden-proposal.json")).unwrap();
        let mut activation: Activation =
            storage::decode(include_bytes!("golden-activation.json")).unwrap();
        let earlier = Ref {
            id: "earlier".into(),
            digest: manifest.baseline.digest.clone(),
        };
        let reference = Ref {
            id: "current".into(),
            digest: manifest.baseline.digest.clone(),
        };
        manifest.activations = vec![earlier.clone(), reference.clone()];
        activation.previous = earlier.clone();
        activation.restores = Some(earlier.clone());
        proposal.expected_active = earlier;
        proposal.expected_baseline = manifest.baseline.clone();
        assert!(bindings(&manifest, &reference, &activation, &proposal).is_ok());
    }
}
