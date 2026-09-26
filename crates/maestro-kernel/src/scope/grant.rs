//! Grants: the rights of principals on scopes, each given or taken back in a
//! write that journals it with its actor, read back into a principal's
//! [`ScopeSet`], and reconciled with [`CONFIG_FILE`] for the local principal.
//!
//! This is the one file of the module that records events, and the module's
//! door re-exports nothing from it: the journal takes the door's types, so a
//! re-export from here would close an import cycle.

use super::{
    config::{CONFIG_FILE, Config, LOCAL},
    path::Scope,
    right::Right,
    set::ScopeSet,
};
use crate::{
    journal::{Event, NewEvent, event},
    store::{self, Database},
};
use rusqlite::{Connection, Transaction, params, types::Type};
use serde_json::json;
use std::collections::BTreeSet;

/// The type of the event of a grant given.
const ADDED: &str = "maestro.kernel.grant.added.v1";
/// The type of the event of a grant taken back.
const REVOKED: &str = "maestro.kernel.grant.revoked.v1";

/// A grant a write gives or takes back: a principal's right on a scope, and
/// who changes it.
#[derive(Debug, Clone, Copy)]
struct Grant<'a> {
    /// The principal that holds the right.
    principal: &'a str,
    /// The scope the right is on, with every scope below it.
    scope: &'a Scope,
    /// The right.
    right: Right,
    /// Who gives or takes it back, as the journal records it.
    actor: &'a str,
}

impl Database {
    /// Grants `principal` the `right` on `scope` and every scope below it, in
    /// a write that journals the grant with `actor`, who gives it, and
    /// returns the event; a grant the principal holds already records
    /// nothing and returns none.
    ///
    /// # Errors
    ///
    /// [`store::Error::Sqlite`] when the database cannot record it.
    pub fn grant(
        &self,
        principal: &str,
        scope: &Scope,
        right: Right,
        actor: &str,
    ) -> Result<Option<Event>, store::Error> {
        let grant = Grant {
            principal,
            scope,
            right,
            actor,
        };
        self.write(|transaction| add(transaction, &grant))
    }

    /// Takes back from `principal` the `right` on `scope`, in a write that
    /// journals the revocation with `actor`, who takes it back, and returns
    /// the event; a grant the principal does not hold records nothing and
    /// returns none. A grant on a scope above `scope` still covers it.
    ///
    /// # Errors
    ///
    /// [`store::Error::Sqlite`] when the database cannot record it.
    pub fn revoke(
        &self,
        principal: &str,
        scope: &Scope,
        right: Right,
        actor: &str,
    ) -> Result<Option<Event>, store::Error> {
        let grant = Grant {
            principal,
            scope,
            right,
            actor,
        };
        self.write(|transaction| remove(transaction, &grant))
    }

    /// The scopes `principal` may read now: those it was granted the read
    /// right on, and every scope below one. Its grants are read anew each
    /// time, so a revocation applies to the next read; a principal with no
    /// grant gets the empty set, which sees nothing.
    ///
    /// # Errors
    ///
    /// [`store::Error::Sqlite`] when the database cannot be read, or holds a
    /// grant whose scope it cannot read back.
    pub fn visible(&self, principal: &str) -> Result<ScopeSet, store::Error> {
        Ok(ScopeSet::new(granted(
            &self.reader()?,
            principal,
            Right::Read,
        )?))
    }

    /// Reconciles the local principal's grants with `config`, in one write:
    /// grants each scope it lists that the principal lacks, then revokes each
    /// one the principal holds that it does not list, each journaled with
    /// [`CONFIG_FILE`] as its actor, and returns their events in that order.
    /// Applying the same configuration again records nothing; one that grants
    /// nothing, as a missing file, revokes every grant of the principal.
    ///
    /// # Errors
    ///
    /// [`store::Error::Sqlite`] when the database cannot be read or written:
    /// nothing changes then.
    pub fn apply_config(&self, config: &Config) -> Result<Vec<Event>, store::Error> {
        self.write(|transaction| {
            let held = granted(transaction, LOCAL, Right::Read)?;
            let added = config
                .read
                .difference(&held)
                .map(|scope| add(transaction, &local(scope)));
            let revoked = held
                .difference(&config.read)
                .map(|scope| remove(transaction, &local(scope)));
            added.chain(revoked).filter_map(Result::transpose).collect()
        })
    }
}

/// The local principal's read right on `scope`, as [`CONFIG_FILE`] gives it
/// or takes it back.
fn local(scope: &Scope) -> Grant<'_> {
    Grant {
        principal: LOCAL,
        scope,
        right: Right::Read,
        actor: CONFIG_FILE,
    }
}

/// Gives `grant` inside `transaction` and journals it there, unless the
/// principal holds it already; returns the event.
///
/// # Errors
///
/// [`store::Error::Sqlite`] when it cannot be recorded: the caller's write
/// then rolls back.
fn add(transaction: &Transaction<'_>, grant: &Grant<'_>) -> Result<Option<Event>, store::Error> {
    let added = transaction.execute(
        "INSERT INTO grants (principal, scope, right, granted_by) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (principal, scope, right) DO NOTHING",
        params![
            grant.principal,
            grant.scope.as_str(),
            grant.right.as_str(),
            grant.actor,
        ],
    )?;
    if added == 0 {
        return Ok(None);
    }
    journal(transaction, grant, ADDED).map(Some)
}

/// Takes `grant` back inside `transaction` and journals it there, unless the
/// principal does not hold it; returns the event.
///
/// # Errors
///
/// [`store::Error::Sqlite`] when it cannot be recorded: the caller's write
/// then rolls back.
fn remove(transaction: &Transaction<'_>, grant: &Grant<'_>) -> Result<Option<Event>, store::Error> {
    let removed = transaction.execute(
        "DELETE FROM grants WHERE principal = ?1 AND scope = ?2 AND right = ?3",
        params![grant.principal, grant.scope.as_str(), grant.right.as_str()],
    )?;
    if removed == 0 {
        return Ok(None);
    }
    journal(transaction, grant, REVOKED).map(Some)
}

/// The scopes `connection` records `principal` holds `right` on.
///
/// # Errors
///
/// [`store::Error::Sqlite`] when the database cannot be read, or holds a
/// grant whose scope it cannot read back.
fn granted(
    connection: &Connection,
    principal: &str,
    right: Right,
) -> Result<BTreeSet<Scope>, store::Error> {
    let mut statement =
        connection.prepare("SELECT scope FROM grants WHERE principal = ?1 AND right = ?2")?;
    let scopes = statement
        .query_map(params![principal, right.as_str()], |row| {
            let path: String = row.get(0)?;
            path.parse().map_err(|invalid| {
                rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(invalid))
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(scopes)
}

/// Journals `grant` inside `transaction` as an event of type `r#type`, on
/// the stream of its principal, in the scope it is on.
fn journal(
    transaction: &Transaction<'_>,
    grant: &Grant<'_>,
    r#type: &str,
) -> Result<Event, store::Error> {
    let stream = format!("principal/{}", grant.principal);
    let data = json!({
        "principal": grant.principal,
        "right": grant.right.as_str(),
        "actor": grant.actor,
    });
    event::record(
        transaction,
        &NewEvent {
            stream: &stream,
            r#type,
            subject: &stream,
            scope: grant.scope.as_str(),
            data: &data,
        },
    )
}
