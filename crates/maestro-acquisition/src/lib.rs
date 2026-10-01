//! Strict source-policy configuration for native acquisition.
//!
//! Resolution reads only admitted immutable resources. It never creates grants,
//! starts transports/sessions, activates connectors or chooses default adapters.
mod files;
pub mod policy;
mod ports;
mod refusal;
/// Checked-address classification and replaceable pinned connections.
pub mod transport;

pub use files::{DirectFiles, LocalResource};
pub use maestro_knowledge::collection::PolicyReference as Ref;
pub use policy::resolve::{parse_policy, validate};
pub use ports::{
    Admission, AdmissionStatus, CheckedPolicy, ImmutableResource, PolicySource, Principal,
    ResourceSource,
};
pub use refusal::Refusal;
