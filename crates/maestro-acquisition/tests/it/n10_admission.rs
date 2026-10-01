//! Robots denial reaches N07's real admission path before transport effects.
use super::{n07_parse_url_identity_and_denial_precedence as n07, support};
use maestro_acquisition::{
    Refusal,
    policy::{
        decision::{AdmissionControls, Request, RequestKind, admit},
        identity::FetchIdentity,
        source::Source,
    },
    transport::{
        pacing::{Demand, OriginLedger, PacingLimits},
        robots::{DenyOverrides, RobotsBinding, RobotsCache},
    },
};

/// Actual N10 rules bound into the existing replaceable admission port.
#[derive(Debug)]
struct Controls<'a> {
    /// Origin-bound rules response from the admitted robots fetch.
    cache: RobotsCache,
    /// Immutable checked source's robots contract.
    policy: RobotsBinding<'a>,
}
impl AdmissionControls for Controls<'_> {
    fn caller(&self, _source: &Source, _request: &Request<'_>) -> Result<(), Refusal> {
        Ok(())
    }
    fn network(&self, _identity: &FetchIdentity) -> Result<(), Refusal> {
        Ok(())
    }
    fn robots(&self, identity: &FetchIdentity) -> Result<(), Refusal> {
        self.cache.check(identity, self.policy, 0, &DenyOverrides)
    }
}
#[test]
fn n10_every_request_kind_robots_denial_has_zero_dispatches() {
    let (mut collection, mut catalog) = n07::fixture();
    let mut wire = support::value(&catalog, "policy");
    wire["sources"][0]["robots"]["agent"] = "Maestro".into();
    support::put(&mut catalog, "policy", &wire);
    support::rebind(&mut collection, &mut catalog);
    let checked = n07::checked(collection, &catalog).unwrap();
    let source = checked.policy().sources.first().unwrap();
    let target = FetchIdentity::parse(source, "https://garden.example/docs/blocked").unwrap();
    let controls = Controls {
        cache: RobotsCache::response(
            &target,
            RobotsBinding::new(&checked, &source.id).unwrap(),
            200,
            b"User-agent: *\nDisallow: /docs/blocked\n",
            0,
        ),
        policy: RobotsBinding::new(&checked, &source.id).unwrap(),
    };
    let ledger = OriginLedger::new(0);
    let effective =
        PacingLimits::compose([&source.limits, &checked.policy().aggregate_limits]).unwrap();
    let mut dispatches = 0;
    for kind in [
        RequestKind::Seed,
        RequestKind::Redirect,
        RequestKind::Subresource,
        RequestKind::Resume,
        RequestKind::Retry,
    ] {
        let mut request = n07::request("https://garden.example/docs/blocked", kind);
        request.cache_bypass = true;
        let admission = admit(&checked, &request, &controls);
        if let Ok(admission) = &admission {
            let _permit = ledger
                .acquire(
                    admission.identity(),
                    effective,
                    Demand::initial("synthetic", 0, 10_000, 100_000),
                )
                .unwrap();
            dispatches += 1;
        }
        assert_eq!(admission.unwrap_err(), Refusal::Access);
    }
    assert_eq!(dispatches, 0);
    let admitted = admit(
        &checked,
        &n07::request("https://garden.example/docs/allowed", RequestKind::Seed),
        &controls,
    )
    .unwrap();
    let _permit = ledger
        .acquire(
            admitted.identity(),
            effective,
            Demand::initial("synthetic", 0, 10_000, 100_000),
        )
        .unwrap();
    dispatches += 1;
    assert_eq!(dispatches, 1);
}
