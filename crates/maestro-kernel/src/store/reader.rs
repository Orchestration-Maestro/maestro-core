//! Short read units borrow configured connections without serializing their queries.
use rusqlite::Connection;
use std::{
    ops::{Deref, DerefMut},
    sync::{Mutex, PoisonError},
};

/// A connection returned to the free list after its read unit ends.
#[derive(Debug)]
pub(crate) struct Reader<'a> {
    /// Owned while borrowed; taken only by drop.
    connection: Option<Connection>,
    /// Idle connections of this database, never locked while querying.
    pool: &'a Mutex<Vec<Connection>>,
}
impl<'a> Reader<'a> {
    /// Borrow an idle reader, opening only when concurrent units need another.
    pub(super) fn borrow<E>(
        pool: &'a Mutex<Vec<Connection>>,
        open: impl FnOnce() -> Result<Connection, E>,
    ) -> Result<Self, E> {
        let idle = pool.lock().unwrap_or_else(PoisonError::into_inner).pop();
        Ok(Self {
            connection: Some(match idle {
                Some(connection) => connection,
                None => open()?,
            }),
            pool,
        })
    }
}
impl Deref for Reader<'_> {
    type Target = Connection;
    #[expect(clippy::expect_used, reason = "only drop takes the owned connection")]
    fn deref(&self) -> &Connection {
        self.connection.as_ref().expect("borrowed reader")
    }
}
impl DerefMut for Reader<'_> {
    #[expect(clippy::expect_used, reason = "only drop takes the owned connection")]
    fn deref_mut(&mut self) -> &mut Connection {
        self.connection.as_mut().expect("borrowed reader")
    }
}
impl Drop for Reader<'_> {
    fn drop(&mut self) {
        if let Some(connection) = self.connection.take() {
            // Never let an unfinished transaction retain a stale snapshot in the pool.
            if !connection.is_autocommit() && connection.execute_batch("ROLLBACK").is_err() {
                return;
            }
            self.pool
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(connection);
        }
    }
}
