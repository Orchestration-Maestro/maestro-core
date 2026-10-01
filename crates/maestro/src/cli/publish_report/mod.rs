//! Historical publication and current projection readiness reporting.

mod render;
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(super) use render::{
    PublishDocument, SCHEMA, generation_alias, generation_collection, generation_label,
    publish_line,
};
pub(super) use render::{ReportRequest, recovery_command, render};
