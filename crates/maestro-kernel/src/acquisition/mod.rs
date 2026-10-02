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

mod capture;
mod envelope;
mod headers;
pub use capture::{CaptureContext, Captures, PreparedCapture};
pub use envelope::{
    CaptureEnvelope, HeaderReason, RedirectHop, Representation, SAFE_HEADERS, SafeHeader,
    SafeIdentity, Transport,
};
pub use headers::{safe_header_names, safe_headers, safe_media};

mod partition;
mod partition_checkpoint;
mod partition_record;
pub use partition::Partitions;
pub use partition_record::{
    AcceptedPartition, Batch, ChangeKeys, DiscoveredItem, Enumeration, NotEnqueued,
    NotEnqueuedReason, Partition, PartitionState, Window,
};

mod link;
pub use link::{MappedRevision, RevisionLink};

mod work_order;
pub use work_order::{WorkCursor, WorkItem};
