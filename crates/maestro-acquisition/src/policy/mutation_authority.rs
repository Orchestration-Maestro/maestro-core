//! Mutation-debt contracts for exact grant bindings and protected diagnostics.
use super::{
    authority::{AuthorityRefusal, Grant, Operation, Permit, Target},
    identity::{DisplayLink, SignedTransferUrl},
};
use crate::Refusal;
use std::time::Duration;

/// Synthetic exact grant; no policy evidence can create this record.
fn grant() -> Grant {
    Grant {
        id: "grant".into(),
        principal: "reader".into(),
        operation: Operation::Fetch,
        target: Target {
            scope: "workspace/default/collection/garden".into(),
            source: "notes".into(),
            account: "public".into(),
            resource: "https://garden.example/docs".into(),
        },
        expires_at: "2026-10-01T00:00:00Z".into(),
    }
}

#[test]
fn policy_authority_each_binding_refuses_independently() {
    let grant = grant();
    let expiry = grant.expiry().unwrap();
    let now = expiry - Duration::from_secs(1);
    assert_eq!(
        grant.decide("reader", Operation::Fetch, &grant.target, now),
        Ok(Permit {
            grant_id: "grant".into()
        })
    );
    for dimension in 0..3 {
        let mut target = grant.target.clone();
        let principal = if dimension == 0 { "other" } else { "reader" };
        let operation = if dimension == 1 {
            Operation::RobotsOverride
        } else {
            Operation::Fetch
        };
        if dimension == 2 {
            target.account = "other".into();
        }
        assert_eq!(
            grant.decide(principal, operation, &target, now),
            Err(AuthorityRefusal::Refused(Refusal::Access)),
            "dimension {dimension}"
        );
    }
    for now in [expiry, expiry + Duration::from_secs(1)] {
        assert_eq!(
            grant.decide("reader", Operation::Fetch, &grant.target, now),
            Err(AuthorityRefusal::Expired {
                grant_id: "grant".into()
            })
        );
    }
}

#[test]
fn policy_authority_invalid_ids_and_targets() {
    let baseline = grant();
    for dimension in 0..2 {
        let mut changed = baseline.clone();
        if dimension == 0 {
            changed.id.clear();
        } else {
            changed.principal.clear();
        }
        assert_eq!(changed.expiry(), Err(Refusal::Invalid));
    }
    for dimension in 0..5 {
        let mut target = baseline.target.clone();
        match dimension {
            0 => target.scope = "invalid".into(),
            1 => target.source.clear(),
            2 => target.resource = "http://garden.example/docs".into(),
            3 => target.resource.push_str("?query=value"),
            _ => target.resource.push_str("#fragment"),
        }
        assert_eq!(
            target.validate(),
            Err(Refusal::Invalid),
            "dimension {dimension}"
        );
    }
}

#[test]
fn policy_protected_diagnostics_and_transfer_bytes() {
    assert_eq!(
        AuthorityRefusal::Expired {
            grant_id: "protected".into()
        }
        .to_string(),
        "authority expired"
    );
    assert_eq!(
        AuthorityRefusal::Refused(Refusal::Access).to_string(),
        Refusal::Access.to_string()
    );
    let input = "https://garden.example/docs?token=synthetic#anchor";
    let display = DisplayLink::parse(input).unwrap();
    let transfer = SignedTransferUrl::parse(input).unwrap();
    assert_eq!(format!("{display:?}"), "DisplayLink([protected])");
    assert_eq!(format!("{transfer:?}"), "SignedTransferUrl([protected])");
    assert_eq!(transfer.as_str(), input);
}
