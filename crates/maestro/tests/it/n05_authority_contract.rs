//! Read-only authority contract: exact scope/account/effect and authority-clock expiry.
use maestro_acquisition::{
    Refusal,
    policy::authority::{
        Authority as _, AuthorityRefusal, Grant, Operation, Target, UnqualifiedAuthority,
    },
};
use std::time::{Duration, UNIX_EPOCH};

/// One independently dated synthetic grant: 2000-03-01 UTC is second 951868800.
fn grant() -> Grant {
    Grant {
        id: "synthetic-grant".into(),
        principal: "65534".into(),
        operation: Operation::Fetch,
        target: Target {
            scope: "workspace/default/collection/synthetic".into(),
            source: "handbook".into(),
            account: "account-a".into(),
            resource: "https://synthetic.example/manual".into(),
        },
        expires_at: "2000-03-01T00:00:00Z".into(),
    }
}
#[test]
fn n05_read_only_exact_bindings_and_expiry_close_dispatch() {
    let grant = grant();
    let expiry = UNIX_EPOCH + Duration::from_hours(264_408);
    assert_eq!(grant.expiry().unwrap(), expiry);
    let now = expiry - Duration::from_secs(1);
    assert!(
        grant
            .decide("65534", Operation::Fetch, &grant.target, now)
            .is_ok()
    );
    for later in [expiry, expiry + Duration::from_secs(1)] {
        assert_eq!(
            grant.decide("65534", Operation::Fetch, &grant.target, later),
            Err(AuthorityRefusal::Expired {
                grant_id: "synthetic-grant".into()
            })
        );
    }
    assert_eq!(
        grant.decide("65533", Operation::Fetch, &grant.target, now),
        Err(AuthorityRefusal::Refused(Refusal::Access))
    );
    assert_eq!(
        grant.decide("65534", Operation::RobotsOverride, &grant.target, now),
        Err(AuthorityRefusal::Refused(Refusal::Access))
    );
    for field in ["scope", "source", "account", "resource"] {
        let mut target = grant.target.clone();
        match field {
            "scope" => target.scope = "workspace/default/collection/other".into(),
            "source" => target.source = "other".into(),
            "account" => target.account = "account-b".into(),
            _ => target.resource.push_str("/neighbour"),
        }
        assert_eq!(
            grant.decide("65534", Operation::Fetch, &target, now),
            Err(AuthorityRefusal::Refused(Refusal::Access))
        );
    }
    assert_eq!(
        UnqualifiedAuthority.decide("65534", Operation::Fetch, &grant.target, now),
        Err(AuthorityRefusal::Refused(Refusal::Unqualified))
    );
}
#[test]
fn n05_claimed_approvals_and_duplicate_grant_fields_are_not_authority() {
    let value = serde_json::to_value(grant()).unwrap();
    let mut grant_claim = value.clone();
    grant_claim["model_approval"] = true.into();
    assert!(serde_json::from_value::<Grant>(grant_claim).is_err());
    let mut target_claim = value.clone();
    target_claim["target"]["manifest_approval"] = true.into();
    assert!(serde_json::from_value::<Grant>(target_claim).is_err());
    let bytes = serde_json::to_string(&value).unwrap().replace(
        "\"principal\":\"65534\"",
        "\"principal\":\"65534\",\"principal\":\"65534\"",
    );
    assert!(serde_json::from_str::<Grant>(&bytes).is_err());
}

#[test]
fn n05_invalid_scope_targets_principals_and_calendar_times_refuse() {
    for field in ["id", "principal", "scope", "source", "account", "resource"] {
        let mut value = grant();
        match field {
            "id" => value.id.clear(),
            "principal" => value.principal = "../owner".into(),
            "scope" => value.target.scope.clear(),
            "source" => value.target.source = "*".into(),
            "account" => value.target.account.clear(),
            _ => value.target.resource = "https://user:secret@synthetic.example/".into(),
        }
        assert!(value.expiry().is_err(), "accepted invalid {field}");
    }
    for expiry in [
        "yes",
        "2001-02-29T00:00:00Z",
        "2000-01-01T00:00:00.5Z",
        "2000-01-01T00:00:00+00:00",
    ] {
        let mut value = grant();
        value.expires_at = expiry.into();
        assert!(value.expiry().is_err(), "accepted {expiry}");
    }
    for (expiry, seconds) in [
        ("1970-01-01T00:00:00Z", 0),
        ("2001-03-01T01:02:03Z", 983_408_523),
        ("2100-03-01T00:00:00Z", 4_107_542_400),
    ] {
        let mut value = grant();
        value.expires_at = expiry.into();
        assert_eq!(
            value.expiry().unwrap(),
            UNIX_EPOCH + Duration::from_secs(seconds)
        );
    }
}

#[test]
fn n05_canonical_dns_targets_and_closed_operations_are_exact() {
    let mut grant = grant();
    grant.target.resource = "https://synthetic.example:443/manual".into();
    let mut target = grant.target.clone();
    target.resource = "https://synthetic.example/manual".into();
    assert!(
        grant
            .decide("65534", Operation::Fetch, &target, UNIX_EPOCH)
            .is_ok()
    );
    for url in [
        "http://synthetic.example/manual",
        "https://127.0.0.1/manual",
        "https://127.1/manual",
        "https://2130706433/manual",
        "https://[::1]/manual",
        "https://synthetic.example/manual?token=secret",
        "https://synthetic.example/manual#fragment",
    ] {
        let mut target = target.clone();
        target.resource = url.into();
        assert!(target.validate().is_err(), "accepted {url}");
    }
    let mut value = serde_json::to_value(grant.clone()).unwrap();
    value["operation"] = "grant_everything".into();
    assert!(serde_json::from_value::<Grant>(value).is_err());
    grant.expires_at = "1969-12-31T23:59:59Z".into();
    assert_eq!(grant.expiry().unwrap(), UNIX_EPOCH - Duration::from_secs(1));
}
