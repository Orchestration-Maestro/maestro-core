//! Private user-local workspace answers replayed from the existing journal.
mod records;
#[cfg(test)]
mod tests;

pub use records::{
    ANSWERED, Answer, Confirmation, ReadOnlyDatabase, WorkspaceAnswer, WorkspaceAuthority,
    WorkspaceRecord,
};
