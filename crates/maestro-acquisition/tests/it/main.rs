//! N03's synthetic integration contracts, in one test binary.
#![expect(clippy::unwrap_used, reason = "test setup must fail loudly")]
mod n03_implement_strict_source_policy_and_local_baseline_resolution;
mod n03_review_fixes;
mod support;
