//! The kernel's database: one SQLite file beside the artifact store, holding
//! every kernel table (docs/architecture/04 §3; plan D1 and D2).
//!
//! The file, `kernel.sqlite3` in the kernel's data directory by default, is in
//! WAL mode. One writer connection, behind a mutex, serves every thread, and
//! each write is one short `IMMEDIATE` transaction; readers open connections
//! of their own, which only read and see the last commit. Every connection
//! waits 5 s for another's lock, enforces foreign keys, and fires the delete
//! triggers of the rows a `REPLACE` removes (recursive triggers), so no
//! replacement that names a guarded row's rowid deletes it. The kernel's
//! other tables write through `Database::write`, in the crate, and pin the
//! artifacts they refer to in the same transaction.
//!
//! `open` creates the file and its missing directories for the owner only,
//! as the artifact store creates its own. A new file is made in WAL mode
//! under the temporary name `<file>.tmp-<process>-<number>`, then hard-linked
//! into place, so processes opening a new database together never race
//! SQLite's switch to WAL: the directory must be on a file system with hard
//! links, as ext4, APFS and NTFS are, and `open` fails naming the file when
//! the link cannot be made. The temporary name is removed whether the link
//! was made or another process made the file first. One left behind, by a
//! crash between the link and the removal or by a removal the system
//! refused, may be another name of the database itself: remove it, never
//! open it, since SQLite would give it a write-ahead log of its own.
//!
//! Migrations are the SQL files of `migrations/`, embedded in the binary,
//! each named after its number, applied once in number order, each in a
//! transaction of its own, and recorded by name in `migrations`. A migration
//! merged later with a lower number still applies; a database that records
//! a migration this binary lacks was migrated by a newer binary and is
//! refused before anything changes. [`pending_migrations`] reads which
//! migrations a database lacks, or the one it records that this binary
//! lacks, from the file opened read-only, and changes nothing.
//!
//! The `artifacts` table records each artifact the store holds: its size, its
//! media type, its pins and when it was first stored. A pin is a record that
//! refers to the artifact, and garbage collection removes only artifacts with
//! none. It lists them first, which is the dry run, deletes their rows in one
//! short transaction, then removes each file in a transaction of its own that
//! first checks no put recorded the artifact again: the one file-system
//! operation the kernel performs inside a transaction, kept to a single
//! unlink. A put stores its bytes, records its row, then stores them again,
//! which writes only a copy a collection removed in between. So a put and a
//! collection, in one process or several and in any order, end with every
//! recorded artifact's file in place, and a crash leaves at worst a file no
//! row names, which a put of the same bytes records again. Files named
//! `.tmp-<process>-<number>` under the artifact root are writes in progress,
//! never artifacts: a collection leaves them alone.

pub(crate) mod artifacts;
mod database;
mod error;
mod migration;
#[cfg(test)]
mod tests;

pub use artifacts::{Artifact, ArtifactCheck};
pub use database::{Database, pending_migrations};
pub use error::Error;
