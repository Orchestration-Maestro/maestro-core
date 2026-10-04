//! Portable state-file names; plan identities inside records keep their original bytes.

/// Encode a validated `sha256:<64 lowercase hex digits>` ID as one portable component.
/// Replacing the fixed separator is injective over that validated ID domain.
fn filename_id(id: &str) -> String {
    id.replace(':', "-")
}

/// Name the write-ahead journal for one immutable plan.
pub(super) fn journal_name(id: &str) -> String {
    format!("journal-{}.toml", filename_id(id))
}

/// Name the committed ownership record for one immutable plan.
pub(super) fn ownership_name(id: &str) -> String {
    format!("ownership-{}.toml", filename_id(id))
}
