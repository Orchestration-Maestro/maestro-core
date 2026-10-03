//! Test-only executed statement counters; no elapsed-time or connection proxy.
use rusqlite::{
    Connection,
    trace::{TraceEvent, TraceEventCodes},
};
use std::cell::Cell;

thread_local! {
    /// Executed statements, isolated from tests on other threads.
    static STATEMENTS: Cell<u64> = const { Cell::new(0) };
    /// Single-item capture-link lookup statements.
    static ITEM_LOOKUPS: Cell<u64> = const { Cell::new(0) };
}

/// Observe executions on writer and pooled reader connections alike.
pub(super) fn install(connection: &Connection) {
    connection.trace_v2(TraceEventCodes::SQLITE_TRACE_STMT, Some(count));
}
/// Count each executed statement, excluding SQLite's trigger-subprogram callbacks.
#[expect(
    clippy::needless_pass_by_value,
    reason = "SQLite trace callback signature owns the event"
)]
fn count(event: TraceEvent<'_>) {
    if let TraceEvent::Stmt(_, sql) = event {
        if sql.starts_with("--") {
            return;
        }
        STATEMENTS.with(|count| count.set(count.get() + 1));
        if sql.contains("WHERE l.item = ?1") {
            ITEM_LOOKUPS.with(|count| count.set(count.get() + 1));
        }
    }
}
/// Executed statements on the current test thread.
pub(super) fn statements() -> u64 {
    STATEMENTS.with(Cell::get)
}
/// Single-item lookup executions on the current test thread.
pub(super) fn item_lookups() -> u64 {
    ITEM_LOOKUPS.with(Cell::get)
}

#[cfg(test)]
mod tests {
    use super::{ITEM_LOOKUPS, STATEMENTS, install, item_lookups, statements};
    use rusqlite::Connection;

    #[test]
    fn k2_statement_counts_nonzero_baseline() {
        STATEMENTS.with(|count| count.set(10));
        ITEM_LOOKUPS.with(|count| count.set(10));
        let connection = Connection::open_in_memory().unwrap();
        install(&connection);
        for _ in 0..3 {
            connection
                .query_row(
                    "SELECT 1 FROM (SELECT 1 AS item) l WHERE l.item = ?1",
                    [1],
                    |_| Ok(()),
                )
                .unwrap();
        }
        assert_eq!(statements(), 13);
        assert_eq!(item_lookups(), 13);
    }
}
