//! N03's synthetic integration contracts, in one test binary.
#![expect(clippy::unwrap_used, reason = "test setup must fail loudly")]
mod n03_implement_strict_source_policy_and_local_baseline_resolution;
mod n03_review_fixes;
mod n07_parse_url_identity_and_denial_precedence;
mod n07_url_policy_edges;
mod n07_url_review_regressions;
mod n08_classify_and_pin_every_destination_address;
mod n09_accounting;
mod n09_authority_connect;
mod n09_boundaries;
mod n09_controls;
mod n09_decode;
mod n09_gzip_members;
mod n09_hop_edges;
mod n09_implement_bounded_admitted_http_transport;
mod n09_interim;
mod n09_pacing;
mod n09_pacing_support;
mod n09_parser;
mod n09_profiles;
mod n09_retained;
mod n09_review_probes;
mod n09_review_support;
mod n09_robots;
mod n09_run_deadline;
mod n09_support;
mod n09_sync_deadlines;
mod n10_admission;
mod n10_conform_robots_and_aggregate_origin_pacing;
mod n10_control_edges;
mod n10_review_regressions;
mod n11_account_aggregate_resources_and_interactive_priority;
mod n11_resource_edges;
mod n11_review_regressions;
mod n11_support;
mod n15_bounds;
mod n15_policy_binding;
mod n15_qualification_closure;
mod n15_resources;
mod n15_review;
mod n15_route_content_through_one_extensible_profile_registry;
mod n15_selection_edges;
mod n15_support;
mod n15_validation;
mod n30_additional_contracts;
mod n30_implement_proposal_and_activation_manifest_write_port;
mod n30_lineage_regressions;
mod n30_mutation_regressions;
mod n30_recovery_guards;
mod n30_review_regressions;
mod n30_support;
mod n30_transition_regressions;
mod n30_write_guards;
mod n32_change_guards;
mod n32_enforce_the_closed_automatic_change_allow_list;
mod n32_support;
mod n57_canonical_artifacts;
mod n57_decisions;
mod n57_decode_bounds;
mod n57_goldens;
mod n57_limits;
mod n57_model_limits;
mod n57_object_refs;
mod n57_processing_artifacts;
mod n57_protected_closure;
mod n57_rollback;
mod n57_selection_guards;
mod n57_support;
mod support;

mod n12_commit_immutable_captures_and_reconciled_run_outcomes;
mod n12_outcomes;
mod n12_support;

mod n12_receipts;

mod n12_golden;

mod n12_edges;

mod n12_secrets;

mod n12_transfer;

mod n12_ports;

mod n12_profile_binding;

mod n12_parents;

mod n13_durably_enumerate_public_links_and_bounded_partitions;
mod n13_edges;

mod n13_capture_edges;

mod n13_dom_query;

mod n13_review_discovery;

mod n13_review_kernel;

mod n16_define_extraction_fidelity_and_cumulative_decode_contracts;
mod n16_physical_order_index;

mod n16_fix_regressions;
mod n16_stage_regressions;

// Other platforms cannot qualify Linux kernel containment.
#[cfg(target_os = "linux")]
mod n17_qualify_linux_parser_process_containment;
// Opt-in test code requires the dedicated delegated host, never silently skips.
#[cfg(all(target_os = "linux", feature = "parser-containment-tests"))]
mod n17_kernel;
#[cfg(all(target_os = "linux", feature = "parser-containment-tests"))]
mod n17_support;

mod n36_complete_full_and_incremental_lifecycle_windows;

mod n36_link_evidence;
mod n36_revision_edges;

mod n37_resume_cancel_and_fence_failed_dependencies;

mod n37_support;

mod n37_crash;
mod n37_prior;

mod n37_fences;

mod n37_dependencies;
