//! Strict source-policy configuration for native acquisition.
//!
//! Resolution reads only admitted immutable resources. It never creates grants,
//! starts transports/sessions, activates connectors or chooses default adapters.
/// Scoped proposal overlays, atomic activation and durable notification recovery.
pub mod adaptation;
/// Immutable scoped capture preparation and truthful run outcomes.
pub mod capture;
/// Offline captured-page enumeration and bounded partition checkpoints.
pub mod discovery;
pub mod extraction;
mod files;
pub mod lifecycle;
pub mod policy;
mod ports;
mod refusal;
/// Checked-address classification, pinned connections, robots rules and shared origin pacing.
pub mod transport;

pub use files::{DirectFiles, LocalResource};
pub use maestro_knowledge::collection::PolicyReference as Ref;
pub use policy::resolve::{parse_policy, validate};
pub use ports::{
    Admission, AdmissionStatus, CheckedPolicy, ImmutableResource, PolicySource, Principal,
    ProfileRegistry, ResourceSource,
};
pub use refusal::Refusal;
