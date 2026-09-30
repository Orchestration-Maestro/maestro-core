//! The Maestro catalog: its shared security limits and the strict checker of
//! its authoring sources. Checking reads files as bounded data; it never runs a
//! template, script or hook.
#![forbid(unsafe_code)]
pub mod limits;
mod model_cards;
pub mod source;
