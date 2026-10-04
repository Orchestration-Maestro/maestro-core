//! Compile-time composition only; every implementation is mutation-owned in its compiled mode.
#[cfg(feature = "engine")]
pub(super) use super::engine::{producer::Session, registry::open as reader_open};
