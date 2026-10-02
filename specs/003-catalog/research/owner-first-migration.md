# Public v4 checker and bootstrap migration evidence

C39 reviews the C30–C37 migration at integration base `6588e70`.
The historical six-rule wording maps to current FR-S3-040–045 in
[spec.md](../spec.md#functional-requirements) and
[D13](../plan.md#d13-manifest-v4-source-contract); the v4 amendment supersedes
the historical layout, not the safety requirements.

## Rule definitions and review boundary

- FR-S3-040, spec lines 965–973: “Discover only the bounded registered v4
  areas”; “Nonempty collections remain unsupported until S6”. D13
  “Tree and ownership” defines the registered tree and explicit assets.
- FR-S3-041, lines 974–982: “Each area MUST have exactly one `package.toml`”;
  “Resource ownership derives from its area”. D13 “Tree and ownership”
  defines owners, maintainers and descriptor protection.
- FR-S3-042, lines 983–994: “All dependencies MUST use declared typed
  qualified IDs”; mandatory common/core/standards are included once. D13
  “Identity, naming and one cutover” defines qualified identity.
- FR-S3-043, lines 995–1001: “A selected package MUST be removable through
  one safe-boundary transaction”. Here only source removal/refusal and core
  byte preservation are migration proofs; the consumer lifecycle transaction
  is not delivered by C33 (see C55 task mapping).
- FR-S3-044, lines 1002–1010: “The S6 private overlay MUST be explicit, pinned
  and additive only”. Public part proven; private overlay deferred to S6.
  No private checkout, content, overlay or authorization proof is claimed.
- FR-S3-045, lines 1011–1018: “Generate `.github/CODEOWNERS` from area
  descriptors only”; “Exact last-match tests MUST prevent a broad rule undoing
  protection”. D13 “Tree and ownership” defines generation order. Identity,
  quorum and repository protection are separate OA1/trusted-CI obligations.

The corresponding plan definitions are D13 “Tree and ownership”, lines
1405–1424 (“Asset inventories are explicit, owner-local files”; “ownership
derives from that descriptor”; broad content rules first, owners-only rules
last), “Resolution and mandatory standards”, lines 1583–1603
(“Standards/common cannot require core or optional packages”; “All standards
are mandatory and non-removable”), the S6 boundary at lines 1641–1647
(“No replacement owner record, public→private dependency, shadow or new grant”;
“Check public alone before combined sources”), and D14 lines 1705–1707
(“Lock/selection/projection/receipt activate atomically at a safe boundary”).

The matrix covers the public checker/bootstrap, not all M3 exits. Native
projection/live formats stay C40/C08; runtime defaults/config/bundle wiring
stays C45a/C46/C47a/C47b. FR-S3-043's lifecycle transaction is a remaining
consumer obligation, not substituted by source-only checks.

## Evidence matrix

Counts below are named-test counts, not assertion counts. Each command selects
only the listed tests; zero tests or absent assertions are findings. Repeated
rows share a proving run and must not be summed as distinct tests.

| Item | Proving file and tests | Command | Passing count |
| --- | --- | --- | ---: |
| FR-S3-040: registered v4 placement, naming and bounded assets | [crates/maestro-catalog/src/source/tests/layout.rs](../../../crates/maestro-catalog/src/source/tests/layout.rs)<br>`v4_area_placement_accepts`<br>`nested_or_unknown_area_refuses`<br>`functional_naming_exception_is_exact`<br>`registered_product_names_and_tokens_refuse_without_self_exemptions`<br>`scoped_assets_are_exact_and_owner_local` | [matrix-01](#matrix-01) | 5 |
| FR-S3-041: area-derived owners, maintainers and delegation | [crates/maestro-catalog/src/source/tests/ownership.rs](../../../crates/maestro-catalog/src/source/tests/ownership.rs)<br>`area_owners_maintainers_validate`<br>`resource_ownership_is_derived`<br>`groups_have_no_inherited_approval_authority`<br>`broad_codeowners_rule_cannot_override_descriptor` | [matrix-02](#matrix-02) | 4 |
| FR-S3-042: layer directions, common passing neighbour and mandatory roots | [crates/maestro-catalog/src/source/tests/accepted.rs](../../../crates/maestro-catalog/src/source/tests/accepted.rs)<br>`explicit_area_layer_matrix_and_core_internal_wiring` | [matrix-03](#matrix-03) | 1 |
| FR-S3-042: common/core/language refusal neighbours | [crates/maestro-catalog/src/source/tests/layer_placements.rs](../../../crates/maestro-catalog/src/source/tests/layer_placements.rs)<br>`common_to_core_refuses`<br>`core_to_team_refuses`<br>`language_to_team_refuses` | [matrix-04](#matrix-04) | 3 |
| FR-S3-042: mandatory standards pinned once | [crates/maestro-catalog/src/source/tests/standards.rs](../../../crates/maestro-catalog/src/source/tests/standards.rs)<br>`every_standard_is_pinned_once`<br>`missing_or_optional_standard_refuses`<br>`standard_removal_refuses` | [matrix-05](#matrix-05) | 3 |
| FR-S3-043: public source removal preserves core; dangling qualified IDs refuse | [crates/maestro-catalog/src/source/tests/references.rs](../../../crates/maestro-catalog/src/source/tests/references.rs)<br>`package_removal_keeps_core_bytes`<br>`removed_package_dangling_reference_refuses` | [matrix-06](#matrix-06) | 2 |
| FR-S3-044: public part proven; private overlay deferred to S6 per spec.md:1002–1010 and FR-S3-040 at 973 | [crates/maestro-catalog/src/source/tests/layout.rs](../../../crates/maestro-catalog/src/source/tests/layout.rs)<br>`unregistered_collection_refuses_without_another_guard` | [matrix-07](#matrix-07) | 1 |
| FR-S3-044: standalone public check needs no private checkout | [crates/maestro/tests/it/catalog_check.rs](../../../crates/maestro/tests/it/catalog_check.rs)<br>`catalog_check_passes_the_valid_catalog` | [matrix-08](#matrix-08) | 1 |
| FR-S3-045: generated CODEOWNERS exact bytes and last-match protection | [crates/maestro-catalog/src/source/tests/codeowners.rs](../../../crates/maestro-catalog/src/source/tests/codeowners.rs)<br>`fixture_codeowners_matches_golden`<br>`descriptor_owner_rule_wins_last`<br>`root_owners_govern_generated_and_shared_files`<br>`codeowners_drift_refuses`<br>`removed_area_rule_refuses` | [matrix-09](#matrix-09) | 5 |
| Qualified IDs: roundtrip, collisions and grammar | [crates/maestro-catalog/src/source/tests/qualified.rs](../../../crates/maestro-catalog/src/source/tests/qualified.rs)<br>`root_and_namespace_ids_roundtrip`<br>`same_stem_different_kind_accepts`<br>`duplicate_kind_namespace_name_refuses`<br>`duplicate_area_namespace_refuses`<br>`identity_serialization_is_a_typed_golden_vector`<br>`qualified_segments_keep_the_64_character_boundary` | [matrix-10](#matrix-10) | 6 |
| maestro-source/2: accepted envelope and /1 or mixed refusal | [crates/maestro-catalog/src/source/tests/qualified.rs](../../../crates/maestro-catalog/src/source/tests/qualified.rs)<br>`root_and_namespace_ids_roundtrip`<br>`old_or_mixed_layout_refuses`<br>`old_and_malformed_ids_refuse_without_rebinding` | [matrix-11](#matrix-11) | 3 |
| maestro-cli/catalog-check/2: exact JSON marker and qualified ID | [crates/maestro/tests/it/catalog_check.rs](../../../crates/maestro/tests/it/catalog_check.rs)<br>`catalog_check_reports_each_resource_under_json` | [matrix-12](#matrix-12) | 1 |
| maestro-project/2: exact descriptor marker/keys | [crates/maestro-catalog/src/bootstrap/tests/project.rs](../../../crates/maestro-catalog/src/bootstrap/tests/project.rs)<br>`base_only_descriptor_has_exact_authoring_keys_and_lock_reference` | [matrix-13](#matrix-13) | 1 |
| maestro-authoring-lock/2: marker and locked output/source bytes | [crates/maestro-catalog/src/bootstrap/tests/project.rs](../../../crates/maestro-catalog/src/bootstrap/tests/project.rs)<br>`authoring_lock_binds_every_generated_file_and_source` | [matrix-14](#matrix-14) | 1 |
| Descriptor agent version 3 | [crates/maestro-catalog/src/source/tests/registry.rs](../../../crates/maestro-catalog/src/source/tests/registry.rs)<br>`builtin_kinds_loaded_from_data_check_like_the_originals` | [matrix-15](#matrix-15) | 1 |
| Descriptor skill version 3 | [crates/maestro-catalog/src/source/tests/registry.rs](../../../crates/maestro-catalog/src/source/tests/registry.rs)<br>`builtin_kinds_loaded_from_data_check_like_the_originals` | [matrix-15](#matrix-15) | 1 |
| Descriptor instructions version 4 | [crates/maestro-catalog/src/source/tests/registry.rs](../../../crates/maestro-catalog/src/source/tests/registry.rs)<br>`builtin_kinds_loaded_from_data_check_like_the_originals` | [matrix-15](#matrix-15) | 1 |
| Descriptor package version 3 | [crates/maestro-catalog/src/source/tests/registry.rs](../../../crates/maestro-catalog/src/source/tests/registry.rs)<br>`builtin_kinds_loaded_from_data_check_like_the_originals` | [matrix-15](#matrix-15) | 1 |
| Descriptor language version 3 | [crates/maestro-catalog/src/source/tests/registry.rs](../../../crates/maestro-catalog/src/source/tests/registry.rs)<br>`builtin_kinds_loaded_from_data_check_like_the_originals` | [matrix-15](#matrix-15) | 1 |
| Descriptor standard version 4 (C81b dd07ac8) | [crates/maestro-catalog/src/source/tests/registry.rs](../../../crates/maestro-catalog/src/source/tests/registry.rs)<br>`builtin_kinds_loaded_from_data_check_like_the_originals` | [matrix-15](#matrix-15) | 1 |
| Descriptor standard-check version 1 | [crates/maestro-catalog/src/source/tests/registry.rs](../../../crates/maestro-catalog/src/source/tests/registry.rs)<br>`builtin_kinds_loaded_from_data_check_like_the_originals` | [matrix-15](#matrix-15) | 1 |
| Descriptor standard-exception version 1 | [crates/maestro-catalog/src/source/tests/registry.rs](../../../crates/maestro-catalog/src/source/tests/registry.rs)<br>`builtin_kinds_loaded_from_data_check_like_the_originals` | [matrix-15](#matrix-15) | 1 |
| Descriptor preset version 3 | [crates/maestro-catalog/src/source/tests/registry.rs](../../../crates/maestro-catalog/src/source/tests/registry.rs)<br>`builtin_kinds_loaded_from_data_check_like_the_originals` | [matrix-15](#matrix-15) | 1 |
| Descriptor bootstrap-inventory version 1 | [crates/maestro-catalog/src/source/tests/registry.rs](../../../crates/maestro-catalog/src/source/tests/registry.rs)<br>`builtin_kinds_loaded_from_data_check_like_the_originals` | [matrix-15](#matrix-15) | 1 |
| Descriptor model-card version 3 | [crates/maestro-catalog/src/source/tests/registry.rs](../../../crates/maestro-catalog/src/source/tests/registry.rs)<br>`builtin_kinds_loaded_from_data_check_like_the_originals` | [matrix-15](#matrix-15) | 1 |
| Migrated source fixture family: agent/native sections and sidecar | [crates/maestro-catalog/src/source/tests/accepted.rs](../../../crates/maestro-catalog/src/source/tests/accepted.rs)<br>`valid_agent_round_trips_its_profile_and_sidecar` | [matrix-16](#matrix-16) | 1 |
| Migrated source fixture family: skill metadata | [crates/maestro-catalog/src/source/tests/accepted.rs](../../../crates/maestro-catalog/src/source/tests/accepted.rs)<br>`valid_skill_reads_its_maestro_metadata_strings` | [matrix-17](#matrix-17) | 1 |
| Migrated source fixture family: instructions/area descriptors/preset | [crates/maestro-catalog/src/source/tests/accepted.rs](../../../crates/maestro-catalog/src/source/tests/accepted.rs)<br>`valid_catalog_passes_with_every_resource_sorted_by_id`<br>`valid_preset_keeps_its_values`<br>`each_resource_lists_the_files_it_owns` | [matrix-18](#matrix-18) | 3 |
| Migrated source fixture family: invalid neighbours | [crates/maestro-catalog/src/source/tests/schema.rs](../../../crates/maestro-catalog/src/source/tests/schema.rs)<br>`schema_stage_owner_rows_and_workflows_are_checked`<br>`agent_body_needs_the_six_fixed_sections_in_order` | [matrix-19](#matrix-19) | 2 |
| Migrated owner-local bootstrap fixture family: common/base + language/rust, strict JSON | [crates/maestro-catalog/src/bootstrap/tests/project.rs](../../../crates/maestro-catalog/src/bootstrap/tests/project.rs)<br>`core_fixture_presets_compose_from_the_replaceable_directory_adapter`<br>`generated_recipe_json_is_strict_and_preset_collision_refuses` | [matrix-20](#matrix-20) | 2 |
| Migrated CODEOWNERS fixture family: CLI drift and exact golden | [crates/maestro/tests/it/catalog_codeowners.rs](../../../crates/maestro/tests/it/catalog_codeowners.rs)<br>`catalog_codeowners_stdout_matches_fixture_golden`<br>`catalog_codeowners_drift_refuses_without_writing` | [matrix-21](#matrix-21) | 2 |
| Migrated model-card fixture family: unchanged kernel identity | [crates/maestro-catalog/src/model_cards/tests.rs](../../../crates/maestro-catalog/src/model_cards/tests.rs)<br>`declaration_preserves_the_kernel_identity_and_declares_version_separately` | [matrix-22](#matrix-22) | 1 |
| Old locks: /1 and genuine previously committed incomplete /2 refuse rebind | [crates/maestro-catalog/src/bootstrap/tests/locks.rs](../../../crates/maestro-catalog/src/bootstrap/tests/locks.rs)<br>`old_authoring_lock_requires_preview`<br>`genuine_committed_old_v2_lock_refuses_rebind` | [matrix-23](#matrix-23) | 2 |
| Complete source locks: IDs/revisions/digests/area roots and stale input refusal | [crates/maestro-catalog/src/bootstrap/tests/locks.rs](../../../crates/maestro-catalog/src/bootstrap/tests/locks.rs)<br>`every_selected_input_is_locked`<br>`changed_source_path_requires_preview`<br>`registered_checked_config_is_locked_and_revalidated`<br>`requires_only_inventory_still_locks_its_assets` | [matrix-24](#matrix-24) | 4 |
| C04 retained: held reads, post-check links and zero-write refusal | [crates/maestro-catalog/src/bootstrap/tests/snapshot.rs](../../../crates/maestro-catalog/src/bootstrap/tests/snapshot.rs)<br>`checked_snapshot_never_reopens_presets_inventories_or_payloads`<br>`post_check_links_refuse_before_writes` | [matrix-25](#matrix-25) | 2 |
| Aggregate source bounds retained | [crates/maestro-catalog/src/source/tests/bounds.rs](../../../crates/maestro-catalog/src/source/tests/bounds.rs)<br>`aggregate_walk_limit_refuses`<br>`aggregate_walk_counts_unchecked_legacy_support_and_depth`<br>`inert_asset_source_bytes_are_bounded_without_parsing` | [matrix-26](#matrix-26) | 3 |
| Settings authority retained, no source migration bypass | [crates/maestro/tests/it/catalog_preferences.rs](../../../crates/maestro/tests/it/catalog_preferences.rs)<br>`catalog_preferences_apply_refuses_without_touching_root_ancestor_or_authority` | [matrix-27](#matrix-27) | 1 |
| Frozen architecture task inventory remains exactly 85 keys | [crates/maestro-catalog/src/source/tests/accepted.rs](../../../crates/maestro-catalog/src/source/tests/accepted.rs)<br>`frozen_rows_equal_the_traceability_inventory` | [matrix-28](#matrix-28) | 1 |
| Migrated bounded fixture family: bytes/depth/resources | [crates/maestro-catalog/src/source/tests/bounds.rs](../../../crates/maestro-catalog/src/source/tests/bounds.rs)<br>`source_file_bytes_boundary`<br>`source_depth_boundary`<br>`yaml_frontmatter_depth_boundary`<br>`catalog_resources_boundary` | [matrix-29](#matrix-29) | 4 |
| Migrated directory fixture family: no execution/no-follow/escapes | [crates/maestro-catalog/src/source/tests/directory.rs](../../../crates/maestro-catalog/src/source/tests/directory.rs)<br>`directory_checks_the_valid_catalog_without_running_its_scripts`<br>`directory_refuses_link_ancestors_and_direct_reads`<br>`directory_refuses_paths_that_leave_or_bypass_the_root` | [matrix-30](#matrix-30) | 3 |
| Migrated data-extension fixture family: glossary/card/delegation/native tool sequences | [crates/maestro-catalog/src/source/tests/extension.rs](../../../crates/maestro-catalog/src/source/tests/extension.rs)<br>`a_glossary_with_a_number_and_a_nested_table_is_one_descriptor`<br>`glossary_neighbours_are_refused_by_the_generic_checks`<br>`a_model_card_kind_is_one_descriptor_read_from_text`<br>`model_card_neighbours_are_refused_by_the_generic_checks`<br>`scoped_tool_fields_keep_native_names_and_ordered_repeated_arguments` | [matrix-31](#matrix-31) | 5 |
| Migrated Git boundary fixture family: root administration excluded; nested administration refused | [crates/maestro-catalog/src/source/tests/git_boundary.rs](../../../crates/maestro-catalog/src/source/tests/git_boundary.rs)<br>`root_git_large_pack_is_outside_source_and_budgets`<br>`nested_git_is_unregistered_even_when_empty` | [matrix-32](#matrix-32) | 2 |
| Migrated scanner fixture family: one counted snapshot and duplicate paths | [crates/maestro-catalog/src/source/tests/scan.rs](../../../crates/maestro-catalog/src/source/tests/scan.rs)<br>`check_parses_only_the_aggregate_counted_snapshot_bytes`<br>`snapshot_refuses_duplicate_source_paths_even_with_identical_bytes` | [matrix-33](#matrix-33) | 2 |
| Migrated hostile fixture family: aliases/stack/cycles/diagnostic bound | [crates/maestro-catalog/src/source/tests/hostile.rs](../../../crates/maestro-catalog/src/source/tests/hostile.rs)<br>`yaml_aliases_expanding_past_twice_the_frontmatter_bytes_are_refused`<br>`a_chain_of_4095_resources_is_checked_on_a_1_mib_stack`<br>`a_complete_graph_of_50_skills_is_one_cycle_diagnostic`<br>`diagnostics_stop_at_1000_with_a_count_of_the_rest` | [matrix-34](#matrix-34) | 4 |
| Migrated native MCP fixture cases: no implicit resource binding | [crates/maestro-catalog/src/source/tests/rulings.rs](../../../crates/maestro-catalog/src/source/tests/rulings.rs)<br>`agents_keep_native_model_and_fail_closed_on_unbound_mcp_names` | [matrix-35](#matrix-35) | 1 |
| Migrated generic schema fixture family: tool grammar/strict shapes | [crates/maestro-catalog/src/source/tests/coverage.rs](../../../crates/maestro-catalog/src/source/tests/coverage.rs)<br>`tool_names_and_lists_are_checked`<br>`folders_outside_the_layout_are_refused`<br>`yaml_shapes_outside_the_value_model_are_refused` | [matrix-36](#matrix-36) | 3 |
| Owner-qualified graph fixture family: positive/refusal IDs | [crates/maestro-catalog/src/graph/tests/topology.rs](../../../crates/maestro-catalog/src/graph/tests/topology.rs)<br>`owner_first_example_references_accept`<br>`legacy_example_references_refuse` | [matrix-37](#matrix-37) | 2 |
| FR-S3-041: malformed ownership, exact area versions and duplicate namespaces refuse | [crates/maestro-catalog/src/source/tests/area_packages.rs](../../../crates/maestro-catalog/src/source/tests/area_packages.rs)<br>`package_fields_and_path_refuse_invalid_neighbours` | [matrix-38](#matrix-38) | 1 |
| FR-S3-042/045: standards narrow settings; local/expired/wider exceptions refuse | [crates/maestro-catalog/src/source/tests/restrictive_standards.rs](../../../crates/maestro-catalog/src/source/tests/restrictive_standards.rs)<br>`standard_constraints_only_narrow`<br>`local_expired_or_wider_exception_refuses`<br>`nonnegotiable_exception_refuses` | [matrix-39](#matrix-39) | 3 |

## Exact focused commands

All commands below exited 0 on Linux. `CARGO_BUILD_JOBS=3` and the
pinned toolbelt were used; dev/test debug info was `line-tables-only`.
The matrix has 49 rows and 39 distinct commands, covering 92 distinct named tests.
Descriptor rows intentionally share one assertion over all 11 versions.

The missing exact descriptor-version proof was a finding, repaired in the
existing round-trip test. Red proof bumped the bootstrap-inventory descriptor
from 1 to 2: the named version assertion failed (1 failed, exit 100). After
`git checkout -- crates/maestro-catalog/src/source/kinds/bootstrap_inventory.rs`,
the same focused test passed (1 passed, exit 0). No production code changed.

### matrix-01

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::v4_area_placement_accepts$/) | test(/::nested_or_unknown_area_refuses$/) | test(/::functional_naming_exception_is_exact$/) | test(/::registered_product_names_and_tokens_refuse_without_self_exemptions$/) | test(/::scoped_assets_are_exact_and_owner_local$/)'
```

### matrix-02

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::area_owners_maintainers_validate$/) | test(/::resource_ownership_is_derived$/) | test(/::groups_have_no_inherited_approval_authority$/) | test(/::broad_codeowners_rule_cannot_override_descriptor$/)'
```

### matrix-03

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::explicit_area_layer_matrix_and_core_internal_wiring$/)'
```

### matrix-04

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::common_to_core_refuses$/) | test(/::core_to_team_refuses$/) | test(/::language_to_team_refuses$/)'
```

### matrix-05

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::every_standard_is_pinned_once$/) | test(/::missing_or_optional_standard_refuses$/) | test(/::standard_removal_refuses$/)'
```

### matrix-06

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::package_removal_keeps_core_bytes$/) | test(/::removed_package_dangling_reference_refuses$/)'
```

### matrix-07

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::unregistered_collection_refuses_without_another_guard$/)'
```

### matrix-08

```sh
capped cargo nextest run -p maestro --locked --no-fail-fast -E 'test(/::catalog_check_passes_the_valid_catalog$/)'
```

### matrix-09

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::fixture_codeowners_matches_golden$/) | test(/::descriptor_owner_rule_wins_last$/) | test(/::root_owners_govern_generated_and_shared_files$/) | test(/::codeowners_drift_refuses$/) | test(/::removed_area_rule_refuses$/)'
```

### matrix-10

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::root_and_namespace_ids_roundtrip$/) | test(/::same_stem_different_kind_accepts$/) | test(/::duplicate_kind_namespace_name_refuses$/) | test(/::duplicate_area_namespace_refuses$/) | test(/::identity_serialization_is_a_typed_golden_vector$/) | test(/::qualified_segments_keep_the_64_character_boundary$/)'
```

### matrix-11

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::root_and_namespace_ids_roundtrip$/) | test(/::old_or_mixed_layout_refuses$/) | test(/::old_and_malformed_ids_refuse_without_rebinding$/)'
```

### matrix-12

```sh
capped cargo nextest run -p maestro --locked --no-fail-fast -E 'test(/::catalog_check_reports_each_resource_under_json$/)'
```

### matrix-13

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::base_only_descriptor_has_exact_authoring_keys_and_lock_reference$/)'
```

### matrix-14

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::authoring_lock_binds_every_generated_file_and_source$/)'
```

### matrix-15

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::builtin_kinds_loaded_from_data_check_like_the_originals$/)'
```

### matrix-16

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::valid_agent_round_trips_its_profile_and_sidecar$/)'
```

### matrix-17

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::valid_skill_reads_its_maestro_metadata_strings$/)'
```

### matrix-18

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::valid_catalog_passes_with_every_resource_sorted_by_id$/) | test(/::valid_preset_keeps_its_values$/) | test(/::each_resource_lists_the_files_it_owns$/)'
```

### matrix-19

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::schema_stage_owner_rows_and_workflows_are_checked$/) | test(/::agent_body_needs_the_six_fixed_sections_in_order$/)'
```

### matrix-20

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::core_fixture_presets_compose_from_the_replaceable_directory_adapter$/) | test(/::generated_recipe_json_is_strict_and_preset_collision_refuses$/)'
```

### matrix-21

```sh
capped cargo nextest run -p maestro --locked --no-fail-fast -E 'test(/::catalog_codeowners_stdout_matches_fixture_golden$/) | test(/::catalog_codeowners_drift_refuses_without_writing$/)'
```

### matrix-22

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::declaration_preserves_the_kernel_identity_and_declares_version_separately$/)'
```

### matrix-23

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::old_authoring_lock_requires_preview$/) | test(/::genuine_committed_old_v2_lock_refuses_rebind$/)'
```

### matrix-24

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::every_selected_input_is_locked$/) | test(/::changed_source_path_requires_preview$/) | test(/::registered_checked_config_is_locked_and_revalidated$/) | test(/::requires_only_inventory_still_locks_its_assets$/)'
```

### matrix-25

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::checked_snapshot_never_reopens_presets_inventories_or_payloads$/) | test(/::post_check_links_refuse_before_writes$/)'
```

### matrix-26

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::aggregate_walk_limit_refuses$/) | test(/::aggregate_walk_counts_unchecked_legacy_support_and_depth$/) | test(/::inert_asset_source_bytes_are_bounded_without_parsing$/)'
```

### matrix-27

```sh
capped cargo nextest run -p maestro --locked --no-fail-fast -E 'test(/::catalog_preferences_apply_refuses_without_touching_root_ancestor_or_authority$/)'
```

### matrix-28

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::frozen_rows_equal_the_traceability_inventory$/)'
```

### matrix-29

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::source_file_bytes_boundary$/) | test(/::source_depth_boundary$/) | test(/::yaml_frontmatter_depth_boundary$/) | test(/::catalog_resources_boundary$/)'
```

### matrix-30

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::directory_checks_the_valid_catalog_without_running_its_scripts$/) | test(/::directory_refuses_link_ancestors_and_direct_reads$/) | test(/::directory_refuses_paths_that_leave_or_bypass_the_root$/)'
```

### matrix-31

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::a_glossary_with_a_number_and_a_nested_table_is_one_descriptor$/) | test(/::glossary_neighbours_are_refused_by_the_generic_checks$/) | test(/::a_model_card_kind_is_one_descriptor_read_from_text$/) | test(/::model_card_neighbours_are_refused_by_the_generic_checks$/) | test(/::scoped_tool_fields_keep_native_names_and_ordered_repeated_arguments$/)'
```

### matrix-32

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::root_git_large_pack_is_outside_source_and_budgets$/) | test(/::nested_git_is_unregistered_even_when_empty$/)'
```

### matrix-33

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::check_parses_only_the_aggregate_counted_snapshot_bytes$/) | test(/::snapshot_refuses_duplicate_source_paths_even_with_identical_bytes$/)'
```

### matrix-34

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::yaml_aliases_expanding_past_twice_the_frontmatter_bytes_are_refused$/) | test(/::a_chain_of_4095_resources_is_checked_on_a_1_mib_stack$/) | test(/::a_complete_graph_of_50_skills_is_one_cycle_diagnostic$/) | test(/::diagnostics_stop_at_1000_with_a_count_of_the_rest$/)'
```

### matrix-35

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::agents_keep_native_model_and_fail_closed_on_unbound_mcp_names$/)'
```

### matrix-36

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::tool_names_and_lists_are_checked$/) | test(/::folders_outside_the_layout_are_refused$/) | test(/::yaml_shapes_outside_the_value_model_are_refused$/)'
```

### matrix-37

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::owner_first_example_references_accept$/) | test(/::legacy_example_references_refuse$/)'
```

### matrix-38

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::package_fields_and_path_refuse_invalid_neighbours$/)'
```

### matrix-39

```sh
capped cargo nextest run -p maestro-catalog --locked --no-fail-fast -E 'test(/::standard_constraints_only_narrow$/) | test(/::local_expired_or_wider_exception_refuses$/) | test(/::nonnegotiable_exception_refuses$/)'
```

## Task and remaining-evidence mappings

| Requirement | Public migration proof | Remaining exit holder |
| --- | --- | --- |
| SC-S3-008 | C39 Linux named tests and native-host proof below | C28 final CI coverage ≥90% overall/≥95% changed lines, zero missed mutants/timeouts |
| SC-S3-009 | C39 frozen 85-key assertion and explicit scope mapping | C29/C38 recovery and C28 final M3 delivered/remaining evidence; this review is not all 85 delivered rows |
| SC-S3-015 | C30 placement; C31 descriptors; C33 layer/removal; C34 ownership; C35 CODEOWNERS; C81a/C81b standards | C79–C81 content consumers; C42 private additions remain S6 |
| SC-S3-016 | C32 identities/source+CLI cutover; C36 owner inventories; C37 project/lock source closure and stale locks; C50 inventory safety | C40/C08 native live proof; C45a/C46/C47a/C47b runtime config/defaults/bundles; C55 lifecycle transaction |

C39 is mapped to these four success criteria in [tasks.md](../tasks.md).
C30–C37 reports/reviews and their integrated fix rounds were checked against
the current tests, not treated as substitutes for these runs.

## Hosted proof and final gates

The disposable push-only branch `test/s3-c39-hosts` proved the final reviewed
source and compact descriptor test, then was deleted (remote and local
branch/worktree). No temporary workflow is part of the C39 commit.

[Final native run 37007445531](https://github.com/Orchestration-Maestro/maestro-core/actions/runs/37007445531)
proved snapshot `6d55e14452c3746b3e70cc1eb75bf529fd3b8024`:

| Native host | Catalog passing | Doctests passing | CLI catalog passing | Ignored tests |
| --- | ---: | ---: | ---: | --- |
| Linux | 467 | 1 | 69 | 1 catalog, 9 CLI |
| macOS | 467 | 1 | 69 | 9 CLI |
| Windows | 455 | 1 | 67 | 9 CLI |

Each host ran `cargo test -p maestro-catalog --locked -- --test-threads=3`
and `cargo test -p maestro --test it --locked catalog_ -- --test-threads=3`;
both commands exited 0. Platform-specific test totals differ by compiled cases.

[Initial pwsh run 37004796203](https://github.com/Orchestration-Maestro/maestro-core/actions/runs/37004796203)
passed Linux/macOS but failed nine Windows settings-discovery tests (446 passed;
CLI skipped after failure). `Get-Acl` could not autoload
`Microsoft.PowerShell.Security` in a `powershell.exe` child inheriting pwsh's
`PSModulePath`. The approved workflow-only
[bash rerun 37005606030](https://github.com/Orchestration-Maestro/maestro-core/actions/runs/37005606030)
passed all hosts (Linux/macOS 467 catalog + 1 doctest + 69 CLI; Windows
455 + 1 + 67). No test was skipped or weakened to obtain green results. The
parent-shell-independent Windows test repair is queued separately for polish,
not silently included as a migration repair. Full failures are retained in the
C39 ledger report.

The initial added version vector pushed `registry.rs` to 512 counted lines;
workspace verification refused it (3,411 passed, 1 conventions test failed).
The final assertion keeps the exact existing kind/order vector and zips each
kind with its actual/expected version, adding only eight lines. The repeated
version-bump proof failed with the kind named; restoration and the size check
both passed. Native execution was repeated on these final test bytes above.

### Local gate results

All commands below exited 0. Org Clippy configuration and the cross-target
compiler wrappers from the lane rules were used; exact environment commands
and normal hook/Markdown/link evidence are recorded in the C39 ledger report.

| Command | Result |
| --- | --- |
| `capped cargo nextest run -p maestro-catalog -p maestro` | 1062 passed, 11 skipped |
| `capped cargo nextest run --workspace --no-fail-fast` | 3412 passed, 29 skipped |
| `capped cargo clippy --workspace --all-targets --locked -- -D warnings` | passed |
| `capped cargo clippy --workspace --all-targets --locked --target x86_64-pc-windows-gnu -- -D warnings` | passed |
| `capped cargo clippy --workspace --all-targets --locked --target aarch64-apple-darwin -- -D warnings` | passed |
| `RUSTDOCFLAGS="-D warnings -D missing_docs" capped cargo doc --workspace --no-deps --locked` | passed |
| `RUSTDOCFLAGS="-D warnings -D missing_docs" capped cargo doc --workspace --no-deps --locked --document-private-items` | passed |
| `capped cargo fmt --all --check` | passed |
| `capped rust-gate architecture --local` | passed |
| `capped rust-gate duplication` | passed |
| `capped rust-gate licenses` | passed |
| `capped rust-gate guide` | passed |

Generated CODEOWNERS has no drift: `fixture_codeowners_matches_golden` and
`catalog_codeowners_stdout_matches_fixture_golden` prove exact generated bytes;
the named drift/last-match refusals are green. Checked-in CORE/fixture CODEOWNERS
bytes did not change. This is synthetic CORE proof, not a MAN publication,
trusted identity/quorum proof or OA1 repository protection.

No S3 coverage/mutation CI evidence exists yet. Both queries exited 0 and
returned `[]`:

```sh
gh run list --branch feat/s3-integration --limit 100 --json databaseId,url,conclusion,name,headSha
gh pr list --head feat/s3-integration --state all --json number,url,state,headRefOid
```

Hosted proof jobs run tests only. No local mutation/coverage run or invented
metric substitutes for SC-S3-008's C28 final-CI bars. Independent review and the
supervisor integration check remain required.
