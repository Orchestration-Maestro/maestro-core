//! Kernel integration tests for the unit-graph wire contract and producer conformance.
mod n04_frontier_ack;
mod n04_frontier_support;
mod n04_persist_frontier_leases_and_fenced_submissions;
mod n06_receipt_review_regressions;
mod n06_store_scoped_receipts_and_content_free_progress_events;
mod unit_graph_code_leadin;
mod unit_graph_nested_producer;
mod unit_graph_producer;
mod unit_graph_wire;

mod n30_filesystem;

mod n14_release_source;

mod n37_capture_integrity;
mod n37_capture_support;
mod n37_revision_links;
