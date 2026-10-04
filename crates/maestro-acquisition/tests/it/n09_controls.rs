//! Caller/network/robots refusals precede any HTTP-owned effects.
use super::n09_support::*;
use maestro_acquisition::{
    Refusal,
    policy::{
        decision::{AdmissionControls, Request},
        identity::FetchIdentity,
        source::Source,
    },
    transport::{http::Failure, stream::Accounting},
};

/// Deny one real shared admission stage, without manipulating the transport.
#[derive(Debug)]
struct Denied(&'static str);
impl Denied {
    /// Content-free current control decision.
    fn check(&self, stage: &str) -> Result<(), Refusal> {
        if self.0 == stage {
            return Err(Refusal::Access);
        }
        Ok(())
    }
}
impl AdmissionControls for Denied {
    fn caller(&self, _: &Source, _: &Request<'_>) -> Result<(), Refusal> {
        self.check("caller")
    }
    fn network(&self, _: &FetchIdentity) -> Result<(), Refusal> {
        self.check("network")
    }
    fn robots(&self, _: &FetchIdentity) -> Result<(), Refusal> {
        self.check("robots")
    }
}
#[test]
fn n09_current_admission_refuses_before_transport_effects() {
    run(async {
        for (robots, stage) in [
            (false, "caller"),
            (false, "network"),
            (false, "robots"),
            (true, "caller"),
            (true, "network"),
        ] {
            let policy = policy();
            let controls = Denied(stage);
            let grants = Grants::default();
            let dns = Dns::default();
            let wire = Wire::default();
            let mut request = fetch(if robots {
                "https://garden.example/robots.txt"
            } else {
                "https://garden.example/docs/start"
            });
            request.robots = robots;
            let mut accounting = Accounting::new(policy.policy().sources[0].limits.clone());
            assert_eq!(
                http(&policy, &controls, &grants, &dns, &wire)
                    .fetch(&request, &mut accounting)
                    .await
                    .unwrap_err(),
                Failure::Admission(Refusal::Access)
            );
            assert_eq!(dns.calls.get(), 0);
            assert_eq!(grants.calls.get(), 0);
            assert!(wire.requests.lock().unwrap().is_empty());
        }
    });
}
