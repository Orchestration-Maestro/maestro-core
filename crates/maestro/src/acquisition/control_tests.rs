//! Shared admission refuses authenticated sources before authority.
use super::flow_edges::{Fixture, clean};

#[test]
fn n14_shared_controls_refuse_authenticated_source_before_authority() {
    use super::{
        controls::Controls,
        controls::request,
        flow_tests::{Grants, fixture_with},
    };
    use maestro_acquisition::{
        Principal,
        policy::{decision::AdmissionControls, resolve::validate},
    };
    use std::env;
    let fixture = Fixture::new(clean);
    let scopes = fixture.db.visible("reader").unwrap();
    let principal = Principal {
        id: "reader",
        platform: env::consts::OS,
        scopes: &scopes,
    };
    let (collection, files) = fixture_with(|policy| {
        policy["sources"][0]["auth_role"] = serde_json::json!("synthetic-account");
    });
    let policy = validate(&files, &collection, &principal).unwrap();
    let grants = Grants::default();
    let controls = Controls::new(
        &policy,
        &grants,
        "reader",
        "workspace/default/collection/garden",
    );
    let source = policy.policy().sources.first().unwrap();
    assert!(
        controls
            .caller(
                source,
                &request(
                    "notes",
                    "https://garden.example/docs/start",
                    "2026-10-02T00:00:00Z"
                )
            )
            .is_err()
    );
    fixture.finish();
}

#[test]
fn debt_readiness_retains_current_caller_refusal_and_wall_clock() {
    use super::{
        controls::{Controls, Readiness, millis, request},
        flow_tests::Grants,
    };
    use maestro_acquisition::{Refusal, policy::decision::AdmissionControls};
    use std::time::{SystemTime, UNIX_EPOCH};
    let fixture = Fixture::new(clean);
    let grants = Grants::default();
    let controls = Controls::new(&fixture.policy, &grants, "reader", fixture.scope.as_str());
    let mut source = fixture.policy.policy().sources[0].clone();
    source.auth_role = Some("unavailable".into());
    assert_eq!(
        Readiness(&controls).caller(
            &source,
            &request(
                "notes",
                "https://garden.example/docs/start",
                "2026-10-02T00:00:00Z"
            )
        ),
        Err(Refusal::Unsupported)
    );
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let observed = u128::from(millis().unwrap());
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    assert!((before..=after).contains(&observed));
    fixture.finish();
}

#[test]
fn debt_readiness_network_is_explicitly_offline() {
    use super::{
        controls::{Controls, Readiness},
        flow_tests::Grants,
    };
    use maestro_acquisition::policy::{decision::AdmissionControls, identity::FetchIdentity};
    let fixture = Fixture::new(clean);
    let authority = Grants::default();
    let controls = Controls::new(
        &fixture.policy,
        &authority,
        "reader",
        fixture.scope.as_str(),
    );
    let source = &fixture.policy.policy().sources[0];
    let identity = FetchIdentity::parse(source, &source.seeds[0]).unwrap();
    assert_eq!(controls.network(&identity), Ok(()));
    assert_eq!(Readiness(&controls).network(&identity), Ok(()));
    fixture.finish();
}

#[test]
fn debt_readiness_rechecks_authority_after_the_preliminary_read() {
    use super::controls::{Controls, decision, request};
    use maestro_acquisition::{
        Refusal,
        policy::authority::{Authority, AuthorityRefusal, Operation, Permit, Target},
    };
    use std::{cell::Cell, time::SystemTime};

    /// The real mandatory authority port revokes between two separate reads.
    #[derive(Debug, Default)]
    struct RevokingAuthority(Cell<usize>);
    impl Authority for RevokingAuthority {
        fn decide(
            &self,
            _: &str,
            _: Operation,
            _: &Target,
            _: SystemTime,
        ) -> Result<Permit, AuthorityRefusal> {
            let read = self.0.get();
            self.0.set(read + 1);
            if read == 0 {
                Ok(Permit {
                    grant_id: "synthetic".into(),
                })
            } else {
                Err(AuthorityRefusal::Refused(Refusal::Access))
            }
        }
    }
    let fixture = Fixture::new(clean);
    let authority = RevokingAuthority::default();
    let controls = Controls::new(
        &fixture.policy,
        &authority,
        "reader",
        fixture.scope.as_str(),
    );
    let candidate = request(
        "notes",
        "https://garden.example/docs/start",
        "2026-10-02T00:00:00Z",
    );
    assert_eq!(
        decision(&fixture.policy, &candidate, &controls),
        "policy_denial"
    );
    assert_eq!(authority.0.get(), 2);
    fixture.finish();
}
