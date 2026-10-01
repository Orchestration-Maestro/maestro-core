//! Immutable captures and content-free distinct-item run outcomes.
mod commit;
mod envelope;
mod outcome;
pub use commit::{CaptureBudget, finish_run, prepare};
pub use envelope::http_envelope;
pub use maestro_kernel::acquisition::{
    CaptureContext, CaptureEnvelope, Captures, HeaderReason, RedirectHop, Representation,
    SafeHeader, SafeIdentity, Transport,
};
pub use outcome::{CountingUnit, Outcome, StageCounts, reconcile};
