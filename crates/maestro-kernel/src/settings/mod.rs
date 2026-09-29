//! The journal of settings changes: every `maestro config set` and `config
//! unset` records who changed which key of which preferences file, from what
//! to what, in the kernel's journal, and `config history` reads them back.
//! The events carry keys and values of preferences only, never question or
//! document text. They are the principal's own: the generic
//! [`Database::events`](crate::store::Database::events) reader never returns
//! them, whatever the reader's grants, and
//! [`Database::setting_changes`](crate::store::Database::setting_changes)
//! reads one principal's stream alone.

mod change;
#[cfg(test)]
mod tests;

pub use change::{CHANGED, RecordedChange, SettingChange};
