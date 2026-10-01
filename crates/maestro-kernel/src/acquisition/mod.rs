//! Kernel-owned acquisition work, exposed through a replaceable frontier port.
mod error;
mod frontier;
mod lease;
mod privacy;
mod receipt;
mod record;

pub use error::Error;
pub use frontier::Frontier;
pub use lease::{DispatchRequest, ItemLease, LeaseRequest, SourceLease};
pub use privacy::{Handle, Progress, ProtectedArtifact, Reason, ReceiptError, Status};
pub use receipt::{
    BudgetUsage, InventoryPage, InventorySchema, ItemDisposition, Receipt, ReceiptSchema, Receipts,
    Stage, StageItem,
};
pub use record::{Item, NewItem};
