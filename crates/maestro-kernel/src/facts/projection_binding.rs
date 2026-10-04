//! Shared immutable projection input vocabulary and safe repair text.
use std::fmt;

/// Repair for incompatible or missing durable projection input pins.
pub const PROJECTION_REBUILD_REPAIR: &str = "keep the files as they are, and rebuild the \
    projection from the kernel's database and artifacts, offline; \
    run maestro knowledge graph rebuild";
/// Supported immutable exact resolver identity, shared by producer and receipt decoder.
pub const EXACT_RESOLVER_VERSION: &str = "maestro-exact-resolution/1";

/// Which frozen projection input no longer matches, never its private value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMismatchKind {
    /// Admitted typed engine settings changed.
    Settings,
    /// The complete frozen authoring lock changed.
    Lock,
    /// The frozen resolution or its resolver differs.
    Resolution,
    /// A legacy native stamp or receipt has no durable input pins.
    Format,
}
impl fmt::Display for InputMismatchKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Settings => "settings",
            Self::Lock => "lock",
            Self::Resolution => "resolution",
            Self::Format => "format",
        })
    }
}
