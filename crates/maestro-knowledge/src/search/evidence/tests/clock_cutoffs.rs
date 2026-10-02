//! Evidence assembly and source loading read their cutoff on the handoff's clock.
//!
//! Just before the cutoff they complete; at the cutoff they time out.

use super::super::{EvidenceCounter, EvidenceError, assemble_evidence, source::SourceCache};
use super::support::{evidence_input, fixture};
use crate::{
    prepare::tests::scratch::revision_of,
    search::tests::clock::{ManualClock, control_at, just_before},
};
use maestro_kernel::retrieval::ReadControl;
use maestro_test_clock::on_stopped_clock;
use std::{
    future, slice,
    sync::{Arc, atomic::AtomicBool},
    time::{Duration, Instant},
};
use tokio::time::Instant as TokioInstant;

const GUIDE: (&str, &str) = ("guide.md", "# Guide\n\nThe canonical source text.\n");

/// A cutoff an hour away: only the clock under test can reach it.
fn cutoff() -> Instant {
    Instant::now() + Duration::from_secs(3600)
}

#[tokio::test]
async fn evidence_assembly_reads_its_cutoff_on_the_handoff_clock() {
    let fixture = fixture(&[GUIDE]);
    on_stopped_clock(future::pending(), || async {
        let mut input = evidence_input(&fixture, "What does the guide say?");
        input.deadline = TokioInstant::now() + Duration::from_secs(3600);
        let cutoff = input.deadline.into_std();
        let mut late = input.clone();
        input.clock = ManualClock::at(just_before(cutoff));
        late.clock = ManualClock::at(cutoff);
        let database = Arc::new(fixture.database);

        assert!(
            assemble_evidence(database.clone(), input, EvidenceCounter::Utf8Bytes)
                .await
                .is_ok()
        );
        assert!(matches!(
            assemble_evidence(database, late, EvidenceCounter::Utf8Bytes).await,
            Err(EvidenceError::TimedOut)
        ));
    })
    .await;
}

#[test]
fn source_loading_reads_its_cutoff_on_the_control_clock() {
    let fixture = fixture(&[GUIDE]);
    let revision = revision_of(&fixture.database, &fixture.scopes, "guide.md");
    let cutoff = cutoff();
    let load_on = |control: &ReadControl| {
        SourceCache::new(&fixture.database, &fixture.scopes, control).load_many_with_workers(
            slice::from_ref(&revision),
            &fixture.generation,
            2,
        )
    };

    assert!(load_on(&control_at(cutoff, just_before(cutoff))).is_ok());
    assert!(matches!(
        load_on(&control_at(cutoff, cutoff)),
        Err(EvidenceError::TimedOut)
    ));
}

#[test]
fn a_cached_source_times_out_exactly_when_its_clock_reaches_the_deadline() {
    let fixture = fixture(&[GUIDE]);
    let revision = revision_of(&fixture.database, &fixture.scopes, "guide.md");
    let cutoff = cutoff();
    let clock = ManualClock::at(just_before(cutoff));
    let control = ReadControl {
        deadline: cutoff,
        clock: clock.clone(),
        cancelled: Arc::new(AtomicBool::new(false)),
    };
    let mut cache = SourceCache::new(&fixture.database, &fixture.scopes, &control);
    assert!(cache.load(&revision, &fixture.generation).is_ok());

    clock.set(cutoff);
    assert!(matches!(
        cache.load(&revision, &fixture.generation),
        Err(EvidenceError::TimedOut)
    ));
}
