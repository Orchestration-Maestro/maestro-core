//! How a check records a finding located at a block's spans.
use crate::content::Block;
use crate::model::{Finding, Severity};

/// Record a finding about a block, located at its spans.
pub(super) fn issue(
    issues: &mut Vec<Finding>,
    block: &Block,
    code: &str,
    message: &str,
    severity: Severity,
) {
    issues.push(Finding {
        severity,
        code: code.into(),
        message: message.into(),
        block_id: Some(block.block_id.clone()),
        source_spans: block.source_spans.clone(),
    });
}
