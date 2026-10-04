//! Build-bound publication proofs on synthetic 0031 databases.
use super::{
    projection_reservation::{begin, lease, request},
    projection_schema_support::Fixture,
    support::timing,
};
use crate::{
    artifact::Digest,
    facts::{
        Error, ProjectionReceipt, projection_publication as publication, projection_reservation,
    },
    job::Lease,
    scope::ScopeSet,
    store::Database,
};
use std::{collections::BTreeSet, sync::Barrier, thread};

/// Native verification output bound to the reserved candidate.
fn receipt(fixture: &Fixture, name: &str) -> ProjectionReceipt {
    let mut receipt = fixture.receipt.clone();
    receipt.identity.file_name = name.into();
    receipt.identity.schema_version = "maestro-typed-edges/3".into();
    receipt
}

/// Publish under a synthetic current lease, committing or rolling back as one unit.
fn publish(
    database: &Database,
    build: i64,
    output: &ProjectionReceipt,
    lease: &Lease,
) -> Result<(), Error> {
    let _ = build;
    database.write(|tx| {
        publication::publish(
            tx,
            &ScopeSet::default_workspace(),
            output,
            lease,
            timing(7).now,
        )
    })
}

#[test]
fn projection_publication_two_live_builds_race_one_predecessor() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    fixture.set_state("published");
    let predecessor = fixture.head();
    let first = request(&fixture, Some(predecessor));
    let mut second = first.clone();
    second.settings_identity = Digest::of(b"other admitted settings");
    let leases = [lease(&fixture, &first), lease(&fixture, &second)];
    let builds = [
        begin(&fixture, &first, &leases[0]).unwrap(),
        begin(&fixture, &second, &leases[1]).unwrap(),
    ];
    let mut receipts = [
        receipt(&fixture, "first.lbdb"),
        receipt(&fixture, "second.lbdb"),
    ];
    for index in 0..2 {
        receipts[index].identity.build_id = builds[index].build_id;
    }
    receipts[1].settings_identity = second.settings_identity;
    let unchanged = [
        "generations",
        "graph_attachments",
        "jobs",
        "events",
        "generation_search",
    ]
    .map(|table| {
        (
            table,
            super::projection_build_upgrade::rows(&fixture, table),
        )
    });
    let barrier = Barrier::new(2);
    let results = thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|index| {
                let fixture = &fixture.database;
                let barrier = &barrier;
                let builds = &builds;
                let receipts = &receipts;
                let leases = &leases;
                scope.spawn(move || {
                    barrier.wait();
                    publish(
                        fixture,
                        builds[index].build_id,
                        &receipts[index],
                        &leases[index],
                    )
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    let winner = results.iter().position(Result::is_ok).unwrap();
    let loser = 1 - winner;
    assert_eq!(fixture.head(), builds[winner].build_id);
    assert_eq!(fixture.count("graph_projection_receipts"), 2);
    let loser_receipts: i64 = fixture
        .connection
        .query_row(
            "SELECT count(*) FROM graph_projection_receipts WHERE build_id = ?1",
            [builds[loser].build_id],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(loser_receipts, 0);
    assert!(
        fixture
            .database
            .write::<_, Error>(|tx| projection_reservation::validate(
                tx,
                &ScopeSet::default_workspace(),
                builds[loser].build_id,
                &leases[loser],
                timing(7).now,
            ))
            .is_err()
    );
    for (table, before) in unchanged {
        assert_eq!(
            super::projection_build_upgrade::rows(&fixture, table),
            before,
            "{table}"
        );
    }
    fixture.foreign_keys_clean();
}

#[test]
fn projection_publication_reservation_strictly_decodes_predecessor_corruption() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    let request = request(&fixture, Some(fixture.head()));
    let lease = lease(&fixture, &request);
    fixture
        .connection
        .execute_batch(
            "DROP TRIGGER graph_projection_receipts_never_changed;
         ",
        )
        .unwrap();
    // Preserve SQL's historical length-only contract, but make the digest nonhex.
    fixture
        .connection
        .execute(
            "UPDATE graph_projection_receipts SET content_digest = ?1",
            ["z".repeat(64)],
        )
        .unwrap();
    assert!(matches!(
        begin(&fixture, &request, &lease),
        Err(Error::Conflict(_))
    ));
    assert_eq!(fixture.count("graph_projection_builds"), 1);
}

#[test]
fn projection_publication_zero_row_head_change_rolls_back_output() {
    for replacement in [false, true] {
        let fixture = Fixture::new(replacement);
        fixture.migrate();
        let previous = replacement.then(|| fixture.head());
        let request = request(&fixture, previous);
        let lease = lease(&fixture, &request);
        let build = begin(&fixture, &request, &lease).unwrap();
        // Force a zero-row result after receipt insertion, not a preflight refusal.
        let operation = if replacement { "UPDATE" } else { "INSERT" };
        fixture
            .connection
            .execute_batch(&format!(
                "CREATE TRIGGER suppress_head BEFORE {operation} ON graph_projection_active
             BEGIN SELECT RAISE(IGNORE); END;"
            ))
            .unwrap();
        let mut output = receipt(&fixture, "candidate.lbdb");
        output.identity.build_id = build.build_id;
        let result = fixture.database.write::<_, Error>(|tx| {
            publication::publish(
                tx,
                &ScopeSet::default_workspace(),
                &output,
                &lease,
                timing(7).now,
            )
        });
        assert!(matches!(result, Err(Error::Conflict(_))));
        assert_eq!(
            fixture.count("graph_projection_receipts"),
            i64::from(replacement)
        );
        assert_eq!(
            fixture.count("graph_projection_active"),
            i64::from(replacement)
        );
        if let Some(previous) = previous {
            assert_eq!(fixture.head(), previous);
        }
        fixture
            .connection
            .execute_batch("DROP TRIGGER suppress_head")
            .unwrap();
        fixture
            .database
            .write::<_, Error>(|tx| {
                publication::publish(
                    tx,
                    &ScopeSet::default_workspace(),
                    &output,
                    &lease,
                    timing(7).now,
                )
            })
            .unwrap();
        assert_eq!(fixture.head(), build.build_id);
        assert!(begin(&fixture, &request, &lease).is_err());
    }
}

#[test]
fn projection_publication_refuses_every_changed_output_pin_and_content() {
    let fixture = Fixture::new(true);
    fixture.migrate();
    let request = request(&fixture, Some(fixture.head()));
    let lease = lease(&fixture, &request);
    let build = begin(&fixture, &request, &lease).unwrap();
    for field in 0..12 {
        let mut output = receipt(&fixture, "candidate.lbdb");
        output.identity.build_id = build.build_id;
        let other = Digest::of(b"not the reserved input");
        match field {
            0 => output.identity.collection_id = "other".into(),
            1 => output.identity.generation_id += 1,
            2 => output.identity.claim_set_id = other,
            3 => output.identity.schema_version = "maestro-typed-edges/2".into(),
            4 => output.resolution_id = other,
            5 => output.resolver_version = "unsupported".into(),
            6 => output.settings_identity = other,
            7 => output.frozen_lock = other,
            8 => output.identity.knowledge_edge_count += 1,
            9 => output.identity.catalog_dependency_edge_count += 1,
            10 => output.identity.entity_fact_count += 1,
            _ => output.identity.content_digest = other,
        }
        assert!(
            matches!(
                fixture
                    .database
                    .write::<_, Error>(|tx| publication::publish(
                        tx,
                        &ScopeSet::default_workspace(),
                        &output,
                        &lease,
                        timing(7).now,
                    )),
                Err(Error::Conflict(_))
            ),
            "field {field}"
        );
        assert_eq!(fixture.count("graph_projection_receipts"), 1);
        assert_eq!(fixture.head(), request.expected_active_build_id.unwrap());
    }
}

#[test]
fn projection_publication_rechecks_live_authority_after_reservation() {
    for boundary in ["denied", "expiry", "foreign", "retired", "failed", "head"] {
        let fixture = Fixture::new(true);
        fixture.migrate();
        let previous = fixture.head();
        let request = request(&fixture, Some(previous));
        let lease = lease(&fixture, &request);
        let build = begin(&fixture, &request, &lease).unwrap();
        let mut output = receipt(&fixture, "candidate.lbdb");
        let mut scopes = ScopeSet::default_workspace();
        let mut now = timing(7).now;
        let mut target = build.build_id;
        match boundary {
            "denied" => scopes = ScopeSet::new(BTreeSet::new()),
            "expiry" => now = timing(66).now,
            "foreign" => target = i64::MAX,
            "head" => {
                let other = fixture.reserve(Some(previous));
                fixture.insert_receipt(other, "winner.lbdb").unwrap();
                fixture.advance(other).unwrap();
            }
            state => fixture.set_state(state),
        }
        // Changing only output cannot conceal stale build authority.
        output.identity.file_name = format!("{boundary}.lbdb");
        output.identity.build_id = target;
        let before = fixture.count("graph_projection_receipts");
        let head = fixture.head();
        assert!(
            fixture
                .database
                .write::<_, Error>(|tx| publication::publish(tx, &scopes, &output, &lease, now,))
                .is_err(),
            "{boundary}"
        );
        assert_eq!(fixture.count("graph_projection_receipts"), before);
        assert_eq!(fixture.head(), head);
    }
}

#[test]
fn projection_publication_predecessor_file_identity_is_strict() {
    for name in ["", ".hidden", "../outside.db", "x/y.db"] {
        let fixture = Fixture::new(true);
        fixture.migrate();
        let request = request(&fixture, Some(fixture.head()));
        let lease = lease(&fixture, &request);
        fixture
            .connection
            .execute_batch(
                "DROP TRIGGER graph_projection_receipts_never_changed;
             PRAGMA ignore_check_constraints=ON;",
            )
            .unwrap();
        fixture
            .connection
            .execute(
                "UPDATE graph_projection_receipts SET file_name = ?1",
                [name],
            )
            .unwrap();
        assert!(
            matches!(begin(&fixture, &request, &lease), Err(Error::Conflict(_))),
            "{name}"
        );
    }
}

#[test]
fn projection_publication_refuses_output_build_from_another_attempt() {
    let fixture = Fixture::new(false);
    fixture.migrate();
    let request = request(&fixture, None);
    let lease = lease(&fixture, &request);
    let build = begin(&fixture, &request, &lease).unwrap();
    let mut output = receipt(&fixture, "candidate.lbdb");
    output.identity.build_id = i64::MAX;
    assert!(publish(&fixture.database, build.build_id, &output, &lease).is_err());
    assert_eq!(fixture.count("graph_projection_receipts"), 0);
}
