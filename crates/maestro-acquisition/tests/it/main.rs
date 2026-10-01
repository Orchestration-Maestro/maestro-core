//! N03's synthetic integration contracts, in one test binary.
#![expect(clippy::unwrap_used, reason = "test setup must fail loudly")]
mod n03_implement_strict_source_policy_and_local_baseline_resolution;
mod n03_review_fixes;
mod n07_parse_url_identity_and_denial_precedence;
mod n07_url_policy_edges;
mod n07_url_review_regressions;
mod n08_classify_and_pin_every_destination_address;
mod n10_admission;
mod n10_conform_robots_and_aggregate_origin_pacing;
mod n10_control_edges;
mod support;
