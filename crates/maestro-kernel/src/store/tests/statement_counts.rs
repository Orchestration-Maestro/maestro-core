//! Executed statement instrumentation observes both pooled readers and writers.
use super::support::Scratch;

#[test]
fn k2_statement_counts_track_each_connection_and_lookup() {
    let scratch = Scratch::new();
    let db = scratch.open();
    let reader = db.reader().unwrap();
    let before = db.statement_count();
    let lookups = db.item_lookup_count();
    for _ in 0..3 {
        reader
            .query_row(
                "SELECT 1 FROM (SELECT 1 AS item) l WHERE l.item = ?1",
                [1],
                |_| Ok(()),
            )
            .unwrap();
    }
    assert_eq!(db.statement_count() - before, 3);
    assert_eq!(db.item_lookup_count() - lookups, 3);
    drop(reader);
    let before = db.statement_count();
    let lookups = db.item_lookup_count();
    db.write::<_, super::super::Error>(|tx| {
        tx.execute_batch("CREATE TABLE counted(value INTEGER); INSERT INTO counted VALUES(1);")?;
        Ok(())
    })
    .unwrap();
    assert_eq!(db.statement_count() - before, 4);
    assert_eq!(db.item_lookup_count(), lookups);
}
