//! The original non-Linux entrypoint error type.
/// Preserve the platform message while sharing the observable binary entry.
pub(crate) type Failure = &'static str;
