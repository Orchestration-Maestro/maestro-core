//! Content-free run summaries; only the owner's offline public preview includes URLs.
#[cfg(any(target_os = "linux", test))]
use maestro_kernel::acquisition::NotEnqueued;
use maestro_kernel::{
    acquisition::{Handle, Status},
    artifact::Digest,
};
use serde::{Deserialize, Serialize};

/// A reference has one disposition and one concrete reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Entry {
    /// Content-free identity of the candidate or captured item.
    pub(crate) reference: Digest,
    /// Fixed explanation, never raw error text.
    pub(crate) reason: String,
    /// Capture observation time, including reuse that was not revalidated.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(crate) observed_ms: Option<u64>,
    /// Public owner preview only; absent in all durable summaries.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(crate) url: Option<String>,
    /// Verification chunk requirement and the effective per-run partition ceiling.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(crate) partition_limit: Option<PartitionLimit>,
}
/// Content-free explanation of a Verification partition hold.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PartitionLimit {
    /// Total Verification chunks needed for this source sweep.
    pub(crate) chunks: u64,
    /// Effective source/aggregate per-run partition ceiling.
    pub(crate) ceiling: u64,
}
impl PartitionLimit {
    /// Explain the durable source hold without leaking source identity.
    fn text(&self) -> String {
        format!(
            concat!(
                "verification needs {} chunks; limits.partitions ceiling {}; ",
                "raise limits.partitions to complete this sweep"
            ),
            self.chunks, self.ceiling,
        )
    }
}
/// Authorized manual capture result, not an embedding or lifecycle claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Report {
    /// Versioned machine output, first in serialized order.
    pub(crate) schema: String,
    /// Complete alone means successful completion.
    pub(crate) status: Status,
    /// Unique logical run, absent during preview.
    pub(crate) run: Option<Handle>,
    /// Unique durable receipt attempt, absent during preview.
    pub(crate) receipt: Option<Handle>,
    /// Offline owner preview decisions; empty in durable capture reports.
    pub(crate) decisions: Vec<Entry>,
    /// Distinct captures, not the sum of HTTP attempts and stages.
    pub(crate) completed: Vec<Entry>,
    /// Every unresolved reference with its hold reason.
    pub(crate) pending: Vec<Entry>,
    /// Exact eligible references beyond bounded inventory, pending at discovery.
    pub(crate) overflow: u64,
    /// Every excluded reference with its exclusion reason.
    pub(crate) discarded: Vec<Entry>,
}
impl Report {
    /// Empty run/preview result, with no implied completed work.
    pub(crate) fn new() -> Self {
        Self {
            schema: "maestro-cli/acquisition/1".into(),
            status: Status::Pending,
            run: None,
            receipt: None,
            decisions: vec![],
            completed: vec![],
            pending: vec![],
            discarded: vec![],
            overflow: 0,
        }
    }
    /// One shared classification of durable N13 evidence.
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn not_enqueued(&mut self, excluded: &NotEnqueued) -> Result<(), serde_json::Error> {
        let reason = serde_json::to_value(excluded.reason)?
            .as_str()
            .unwrap_or("unresolved_identity")
            .to_owned();
        let entry = Entry {
            reference: excluded.reference.clone(),
            reason,
            url: None,
            observed_ms: None,
            partition_limit: None,
        };
        if excluded.reason.pending() {
            self.pending.push(entry);
        } else {
            self.discarded.push(entry);
        }
        Ok(())
    }
    /// Stable distinct-reference summaries, not dispatch attempt counts.
    #[cfg(any(target_os = "linux", test))]
    pub(crate) fn deduplicate(&mut self) {
        for entries in [&mut self.completed, &mut self.pending, &mut self.discarded] {
            entries.sort_by(|left, right| {
                left.reference
                    .as_str()
                    .cmp(right.reference.as_str())
                    .then(left.reason.cmp(&right.reason))
            });
            entries.dedup();
        }
    }
    /// Plain text retains every reason, with no hidden partial work.
    pub(crate) fn text(&self) -> String {
        if self.run.is_none() {
            let mut lines = vec![format!(
                "acquisition preview: {} decisions (no fetches)",
                self.decisions.len()
            )];
            for entry in &self.decisions {
                lines.push(format!(
                    "{}: {}",
                    entry.url.as_deref().unwrap_or(entry.reference.as_str()),
                    entry.reason
                ));
            }
            return lines.join("\n");
        }

        let mut lines = vec![format!(
            concat!(
                "acquisition {:?}: {} verified byte captures, ",
                "{} pending references, {} discarded references"
            ),
            self.status,
            self.completed.len(),
            self.pending.len(),
            self.discarded.len()
        )];
        if self.overflow > 0 {
            lines.push(format!(
                "{} links pending: inventory limit reached; rerun to continue",
                self.overflow
            ));
        }
        if let Some(receipt) = self.receipt {
            lines.push(format!("receipt {receipt}"));
        }
        for (disposition, entries) in [
            ("completed", &self.completed),
            ("pending", &self.pending),
            ("discarded", &self.discarded),
        ] {
            for entry in entries {
                lines.extend(entry.partition_limit.iter().map(PartitionLimit::text));
                lines.push(format!(
                    "{disposition}: {} ({})",
                    entry.url.as_deref().unwrap_or(entry.reference.as_str()),
                    entry.reason
                ));
            }
        }
        lines.join("\n")
    }
}

impl Entry {
    /// Fixed content-free disposition for a current candidate.
    pub(crate) fn new(reference: &str, reason: &str) -> Self {
        Self {
            reference: Digest::of(reference.as_bytes()),
            reason: reason.into(),
            url: None,
            observed_ms: None,
            partition_limit: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Entry, Report};
    use maestro_kernel::acquisition::Handle;

    #[test]
    fn debt_report_deduplicates_and_sorts_each_disposition_without_merging_reasons() {
        let mut report = Report::new();
        let left = Entry::new("a", "captured");
        let right = Entry::new("b", "held");
        for entries in [
            &mut report.completed,
            &mut report.pending,
            &mut report.discarded,
        ] {
            entries.extend([right.clone(), left.clone(), right.clone()]);
        }
        report.deduplicate();
        let mut expected = vec![left, right];
        expected.sort_by(|left, right| left.reference.as_str().cmp(right.reference.as_str()));
        assert_eq!(report.completed, expected);
        assert_eq!(report.pending, expected);
        assert_eq!(report.discarded, expected);
    }
    #[test]
    fn debt_report_zero_overflow_has_no_inventory_limit_line() {
        let mut report = Report::new();
        report.run = Some(Handle::new());
        assert!(!report.text().contains("inventory limit reached"));
        report.overflow = 1;
        assert!(
            report
                .text()
                .contains("1 links pending: inventory limit reached")
        );
    }
}
