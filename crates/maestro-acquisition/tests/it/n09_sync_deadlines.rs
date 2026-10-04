//! Synchronous ports cannot consume a document deadline then dial a socket.
use super::{n07_parse_url_identity_and_denial_precedence::Controls, n09_support::*};
use maestro_acquisition::{
    Refusal,
    policy::authority::{Authority, AuthorityRefusal, Operation, Permit, Target},
    transport::{
        connect::Resolver,
        http::{Failure, Http},
        stream::Accounting,
    },
};
use maestro_kernel::retrieval::Clock;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime},
};
use tokio::time::Instant as TokioInstant;

/// Trusted monotonic time advances inside a synchronous adapter without sleeping.
#[derive(Debug)]
struct ManualClock(Mutex<Instant>);
impl Clock for ManualClock {
    fn now(&self) -> Instant {
        *self.0.lock().unwrap()
    }
}
/// DNS consumes the complete document deadline, but still returns public addresses.
#[derive(Debug)]
struct SlowDns(Arc<ManualClock>);
impl Resolver for SlowDns {
    fn resolve(&self, _: &str) -> Result<Vec<String>, Refusal> {
        *self.0.0.lock().unwrap() += Duration::from_mins(2);
        Ok(vec!["8.8.8.8".into()])
    }
}
/// Authority consumes the complete deadline and returns a now-too-late permit.
#[derive(Debug)]
struct SlowAuthority(Arc<ManualClock>);
impl Authority for SlowAuthority {
    fn decide(
        &self,
        _: &str,
        _: Operation,
        _: &Target,
        _: SystemTime,
    ) -> Result<Permit, AuthorityRefusal> {
        *self.0.0.lock().unwrap() += Duration::from_mins(2);
        Ok(Permit {
            grant_id: "late".into(),
        })
    }
}
#[test]
fn n09_synchronous_resolver_deadline_prevents_dial() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let grants = Grants::default();
        let wire = Wire::new(vec![response(200, "", b"ok")]);
        let clock = Arc::new(ManualClock(Mutex::new(TokioInstant::now().into_std())));
        let dns = SlowDns(clock.clone());
        let mut accounting =
            Accounting::with_clock(policy.policy().sources[0].limits.clone(), clock);
        let client = Http {
            policy: &policy,
            controls: &controls,
            authority: &grants,
            resolver: &dns,
            transport: &wire,
            pacing: &ALLOW_PACING,
            pacing_context: pacing_context(),
        };
        assert_eq!(
            client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Timeout
        );
        assert_eq!(grants.calls.get(), 0);
        assert!(wire.requests.lock().unwrap().is_empty());
        assert_eq!(
            wire.responses.lock().unwrap().len(),
            1,
            "no dial after cutoff"
        );
    });
}
#[test]
fn n09_synchronous_authority_deadline_prevents_dial() {
    run(async {
        let policy = policy();
        let controls = Controls::default();
        let dns = Dns::default();
        let wire = Wire::new(vec![response(200, "", b"ok")]);
        let clock = Arc::new(ManualClock(Mutex::new(TokioInstant::now().into_std())));
        let grants = SlowAuthority(clock.clone());
        let mut accounting =
            Accounting::with_clock(policy.policy().sources[0].limits.clone(), clock);
        let client = Http {
            policy: &policy,
            controls: &controls,
            authority: &grants,
            resolver: &dns,
            transport: &wire,
            pacing: &ALLOW_PACING,
            pacing_context: pacing_context(),
        };
        assert_eq!(
            client
                .fetch(&fetch("https://garden.example/docs/start"), &mut accounting)
                .await
                .unwrap_err(),
            Failure::Timeout
        );
        assert!(wire.requests.lock().unwrap().is_empty());
        assert_eq!(
            wire.responses.lock().unwrap().len(),
            1,
            "no dial after cutoff"
        );
    });
}
