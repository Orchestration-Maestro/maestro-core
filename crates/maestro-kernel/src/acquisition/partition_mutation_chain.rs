//! Replay, continuation, inventory and optimistic-read fencing regressions.
use super::{
    Frontier, Partitions, ReceiptError,
    partition_checkpoint::checkpoint_with,
    partition_mutation_support::{Fixture, batch, discovered, lease, now},
};
use rusqlite::params;
use serde_json::json;

#[test]
fn k1_checkpoint_replay_and_continuation() {
    let fixture = Fixture::new();
    let mut first = batch();
    first.terminal = false;
    first.next = Some(json!(1));
    first.partition.max_batches = 3;
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &first, now())
            .is_ok()
    );
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &first, now())
            .is_ok()
    );
    let mut changed = first.clone();
    changed.stable = false;
    assert!(matches!(
        fixture.db.checkpoint(&fixture.writer, &changed, now()),
        Err(ReceiptError::Conflict)
    ));
    let mut second = first.clone();
    second.cursor = first.next.clone();
    second.next = Some(json!(2));
    let mut wrong_descriptor = second.clone();
    wrong_descriptor.partition.window.overlap = 1;
    assert!(matches!(
        fixture
            .db
            .checkpoint(&fixture.writer, &wrong_descriptor, now()),
        Err(ReceiptError::Conflict)
    ));
    let mut wrong_cursor = second.clone();
    wrong_cursor.cursor = Some(json!(9));
    assert!(matches!(
        fixture.db.checkpoint(&fixture.writer, &wrong_cursor, now()),
        Err(ReceiptError::Conflict)
    ));
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &second, now())
            .is_ok()
    );
    let mut third = second;
    third.cursor = third.next.clone();
    third.next = None;
    third.terminal = true;
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &third, now())
            .is_ok()
    );
    assert_eq!(
        fixture
            .db
            .partition(&fixture.scope, first.partition.id)
            .unwrap()
            .unwrap()
            .batches
            .len(),
        3
    );
}

#[test]
fn k1_terminal_replay_and_append_invariants() {
    let fixture = Fixture::new();
    let terminal = batch();
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &terminal, now())
            .is_ok()
    );
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &terminal, now())
            .is_ok()
    );
    let mut append = terminal.clone();
    append.cursor = Some(json!(1));
    assert!(matches!(
        fixture.db.checkpoint(&fixture.writer, &append, now()),
        Err(ReceiptError::Conflict)
    ));
    fixture
        .db
        .commit_partition(&fixture.writer, terminal.partition.id, now())
        .unwrap();
    assert!(matches!(
        fixture.db.checkpoint(&fixture.writer, &append, now()),
        Err(ReceiptError::Conflict)
    ));
    let mut bad_terminal = batch();
    bad_terminal.next = Some(json!(1));
    assert!(matches!(
        fixture.db.checkpoint(&fixture.writer, &bad_terminal, now()),
        Err(ReceiptError::Invalid)
    ));
}

#[test]
fn k1_batch_and_distinct_inventory_ceilings() {
    let fixture = Fixture::new();
    let mut first = batch();
    first.partition.max_batches = 1;
    first.terminal = false;
    first.next = Some(json!(1));
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &first, now())
            .is_ok()
    );
    let mut second = first.clone();
    second.cursor = first.next.clone();
    second.next = None;
    second.terminal = true;
    assert!(matches!(
        fixture.db.checkpoint(&fixture.writer, &second, now()),
        Err(ReceiptError::Conflict)
    ));
    let mut first = batch();
    first.partition.max_items = 1;
    first.items = vec![discovered(0)];
    first.terminal = false;
    first.next = Some(json!(1));
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &first, now())
            .is_ok()
    );
    let mut second = first.clone();
    second.cursor = first.next.clone();
    second.next = None;
    second.terminal = true;
    second.items = vec![discovered(1)];
    assert!(matches!(
        fixture.db.checkpoint(&fixture.writer, &second, now()),
        Err(ReceiptError::Conflict)
    ));
    second.items = first.items.clone();
    assert!(
        fixture
            .db
            .checkpoint(&fixture.writer, &second, now())
            .is_ok()
    );
    assert_eq!(
        fixture
            .db
            .partition(&fixture.scope, first.partition.id)
            .unwrap()
            .unwrap()
            .batches
            .len(),
        2
    );
}

#[test]
fn k1_checkpoint_transaction_rechecks_source_and_scope() {
    for (source, scope) in [
        ("other", "workspace/default/collection/partition"),
        ("docs", "workspace/default/collection/other"),
    ] {
        let fixture = Fixture::new();
        fixture
            .db
            .lease_source("other", &fixture.scope, lease())
            .unwrap();
        let pending = batch();
        let result = checkpoint_with(&fixture.db, &fixture.writer, &pending, now(), || {
            fixture
                .db
                .write::<_, ReceiptError>(|tx| {
                    tx.execute(
                        "INSERT INTO acquisition_partitions (id, source, scope)
                    VALUES (?1, ?2, ?3)",
                        params![pending.partition.id.to_string(), source, scope],
                    )?;
                    Ok(())
                })
                .unwrap();
        });
        assert!(
            matches!(result, Err(ReceiptError::Conflict)),
            "race {source}/{scope}"
        );
        let count: u32 = fixture
            .db
            .reader()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM acquisition_partition_batches",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "refused race retained a checkpoint");
    }
}
