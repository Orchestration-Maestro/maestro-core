//! Kernel-owned acquisition work, exposed through a replaceable frontier port.
mod error;
mod frontier;
mod lease;
mod record;

pub use error::Error;
pub use frontier::Frontier;
pub use lease::{DispatchRequest, ItemLease, LeaseRequest, SourceLease};
pub use record::{Item, NewItem};
