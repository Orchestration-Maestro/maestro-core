# Implementation Plan: Foundation

**Branch**: `000-foundation` | **Date**: 2026-09-24 | **Spec**: [spec.md](spec.md)

**Input**: the spec, [06 S0](../../docs/architecture/06-roadmap.md#s0-foundation) and
the working tree as measured on 2026-09-24. Tasks: [tasks.md](tasks.md).

## Summary

Turn the uncommitted `maestro-core` working tree into a public repository whose
CI passes the organization's gates, carrying only the canonicalization crate,
renamed `maestro-canonicalization` to follow the workspace's `maestro-<area>`
names (its identities never include the name).
The crate inherits the workspace lints, its four oversized files become modules,
its eleven oversized functions are split, and every item is documented. The native
tokenizer stops reading machine paths from a committed file: a local binding
names them. The repository's own policies become tests CI runs. Then
`maestro-model-router`, already published, is checked and the private collection created.

## Technical Context

**Language/Version**: Rust 1.98.1 (edition 2024) from `rust-toolchain.toml`; the
crate keeps its declared MSRV, 1.85.

**Primary Dependencies**: unchanged for the crate (`pulldown-cmark` =0.13.4,
`rustix` =1.1.5, `serde`, `serde_json`, `serde_yaml_ng` 0.10, `sha2` 0.10). The
policy crate uses `pulldown-cmark`, already in the lockfile.

**Storage**: none. **Testing**: `cargo test`, `cargo llvm-cov`, `cargo mutants`;
four native tests run explicitly with local artifacts.

**Target Platform**: Linux x86_64 (CI `ubuntu-24.04`, reference workstation WSL2).

**Project Type**: Rust library and command-line tool in a Cargo workspace.

**Constraints**: public repository, so no personal path, secret or
vendor-private material; the organization's rulesets (pull request, squash,
signed commits, conventional titles, CodeQL, `rust / Required Rust CI`).

**Scale/Scope**: about 9,000 lines of Rust, 95 tests plus 4 native tests.

## Starting point (measured 2026-09-24)

| Measure | Value |
| --- | --- |
| Tests | 95 pass, 4 native ignored; the 4 pass in 223 s with the local artifacts |
| Line coverage | 92.28 % (`validate.rs` 73.7 %, `tokenizer.rs` 86.0 %) |
| Clippy with the workspace lints inherited | 244 findings: 186 missing docs, 16 on the 11 functions over 100 lines or complexity 15, 1 on a function with six parameters, 41 others |
| Integration tests with the workspace lints | Do not compile: crate docs missing, and helper `unwrap()` calls outside `#[test]` functions |
| Files over 500 counted lines | `chunk_split.rs` 1,419, `chunks.rs` 1,048, `chunk_mapping.rs` 667, `validate.rs` 620; `tokenizer.rs` 499 |
| Mutants | 810 (`chunk_split.rs` 269, `validate.rs` 115, `chunk_mapping.rs` 77, `tokenizer.rs` 70) |
| Personal paths | 25 lines in `tokenizer-contract.json` (recounted in T005; the first count read 21), 4 in `TOKENIZER.md`; the counter's RUNPATH names a build directory |
| Draft CI | Zero-SHA pin, `sonar-analysis: true` and a Sonar token |
| Crate leftovers | Its own `Cargo.lock`, `deny.toml`, `rust-toolchain.toml`, `LICENSE`, `.gitignore`, and a justfile whose `golden` recipe sets `ARTIFACTORY_INDEX` and coverage 80 |
| Documents that do not carry over | `docs/providers/` (59 MB), `docs/superpowers/` (528 KB) |
| Other | README in French; toolbelt pins older than rust-workflows' (just 1.40.0 against 1.58.0, cargo-mutants 25.3.1 against 27.1.0) |

## Constitution Check

| Rule | Status |
| --- | --- |
| I Evidence over assertion | Pass: every claim above is a command output; the native run stays explicit and is reported as run or not run |
| II Vertical slices, no scaffolding | Pass: no empty crate; `maestro-conventions` holds real tests; `maestro-manifests` waits for its first file (S3) |
| III Test-first, deep modules | Pass: the binding and the policies start from failing tests; splits and extractions are guarded by the unchanged suite and fixture bytes |
| IV Provenance and immutability | Pass: canonical identities are unchanged; chunk identities change once, by design, because they include the tokenizer profile's identifier |
| V The host decides | Not applicable in S0 |
| VI Rust first | Pass: the crate's two Python scripts leave the public repository |
| VII No silent degradation | Pass: a missing or invalid binding is a refusal naming the variable and schema |
| VIII Measured | Pass: estimates come from the measurements above |
| Quality gates | Pass, with one recorded exception (Complexity Tracking) |

## Project Structure

### Documentation (this feature)

```text
specs/000-foundation/
├── spec.md
├── plan.md      # this file
└── tasks.md
```

### Source code (repository root, after S0)

```text
maestro-core/
├── .github/
│   ├── CODEOWNERS
│   ├── dependabot.yml
│   ├── prompts/                    # Spec Kit commands for Copilot
│   └── workflows/{ci,scorecard,dependabot-auto-merge}.yml
├── .specify/                       # constitution, Spec Kit templates and scripts
├── crates/
│   ├── maestro-canonicalization/
│   │   ├── src/
│   │   │   ├── chunk_split/{mod,layout,context,prepare,tests}.rs
│   │   │   ├── chunks/{mod,identity,validation}.rs, chunks/tests/{mod,replay}.rs
│   │   │   ├── chunk_mapping/{mod,slice,tests}.rs
│   │   │   ├── validate/{mod,blocks,sections}.rs
│   │   │   ├── tokenizer/{mod,binding,process,tests}.rs
│   │   │   └── accounting, assemble, content, dedup, lib, main, metadata, model, parse, store .rs
│   │   ├── tests/                  # unchanged scenarios, each file with crate docs
│   │   ├── examples/               # fixtures, byte-identical
│   │   ├── tokenizer-contract.json # profile, schema local-tokenizer-contract/2, no paths
│   │   └── README, ACCEPTANCE, CHUNKING, DEDUPLICATION, TOKENIZER, VERIFICATION .md
│   └── maestro-conventions/
│       ├── src/lib.rs              # tracked files, counted lines, Markdown links
│       └── tests/policies.rs
├── docs/{architecture,adr}/
├── specs/{000-foundation,001-knowledge-kernel}/
├── scripts/bootstrap.sh
├── AGENTS.md  CONTEXT.md  README.md  LICENSE  THIRD-PARTY-NOTICES.md
├── Cargo.toml  Cargo.lock  rust-toolchain.toml  clippy.toml  deny.toml  typos.toml
├── mise.toml  mise.lock  justfile  .pre-commit-config.yaml
└── .editorconfig  .gitattributes  .gitignore  .taplo.toml  .yamlfmt.yml
```

**Structure decision**: one Cargo workspace; product code in
`maestro-canonicalization`, repository policies in `maestro-conventions`.

### What leaves the tree

| Path | Destination |
| --- | --- |
| `docs/superpowers/`, `docs/providers/` | The archive snapshot; files naming vendor hosts, entitlements or connectors are also copied to `ctm-collection` |
| `crates/maestro-canonicalization/scripts/verify_corpus.py` | `ctm-collection`: it samples the vendor corpus |
| `crates/maestro-canonicalization/scripts/qualify_tokenizer.py` | The archive; S1's parity qualification replaces it (ADR-0008) |
| The crate's `Cargo.lock`, `deny.toml`, `rust-toolchain.toml`, `LICENSE`, `.gitignore`, `justfile` | Deleted: the workspace root's copies govern |
| `scripts/bootstrap.sh`, `.github/workflows/ci.yml` (drafts) | Replaced |

## Design

### D1 The native binding

| Part | Decision |
| --- | --- |
| Profile | `tokenizer-contract.json` keeps its name; schema `local-tokenizer-contract/2`; `model` and `counter` keep `bytes` and `sha256` only; each library and source is `{file, bytes, sha256}`, a library by file name, a source relative to the llama.cpp source root. New identifier: `sha256:3546447555757daa4996a2e2e708bc67bce4389e8cee3f8386ed503eeaa6d01c` (digest of the sorted fields, as `parse_contract` computes) |
| Binding | A local JSON file outside the repository, named by `MAESTRO_NATIVE_BINDING`: `{"schema": "maestro-native-binding/1", "model", "counter", "library_directory", "source_root"}`; at most 1 MiB; unknown keys refused; a relative path resolves against the binding file's directory |
| Refusals | Unset variable, unreadable or oversized file, invalid JSON, wrong schema or unknown key: each an `Error` naming `MAESTRO_NATIVE_BINDING` and the schema, never a path's contents |
| Library loading | The counter's RUNPATH names an absolute build directory, so the invocation adds `LD_LIBRARY_PATH=<library_directory>`, which the loader searches first: the libraries verified are the libraries loaded |
| API | `NativeTokenizer::open()` keeps its signature and reads the binding; `NativeBinding` is crate-private; `Error` is unchanged |
| Identity | Canonical document and block identities are unchanged. Chunk and prepared-input identities include the profile identifier, so they change once with schema 2; a relocation under the same profile keeps them (explicit native test) |
| Module | `tokenizer.rs` (499 counted lines) becomes `tokenizer/{mod,binding,process,tests}.rs` |

### D2 Module splits (moves only; every test unchanged)

| File | Modules |
| --- | --- |
| `chunk_split.rs` | `mod.rs`: `structure_error`, `layout`, `validate_preparation`, `build_drafts`, `combine`, the types · `layout.rs`: `owner` through `owned_units` · `context.rs`: `context_entries` through `item_prefix` · `prepare.rs`: `body_parts` through `fit_prefix` · `tests.rs`: the test module and `structural_chunks` |
| `chunks.rs` | `mod.rs`: the public types, `chunk_documents`, `chunk_with_count`, `build_batch`, `record_bytes`, `invalid_chunks` · `identity.rs`: `chunk_id`, `prepared_identity`, `insert_prepared_group` · `validation.rs`: `validate_coverage`, `validate_chunks` and its helpers · `tests/mod.rs`, with the replay tests in `tests/replay.rs` |
| `chunk_mapping.rs` | `mod.rs`: `invalid_mapping`, `map_document`, the mapper (`walk` through `new_unit`) · `slice.rs`: `mapped_slice`, `map_accounting` · `tests.rs` |
| `validate.rs` | `mod.rs`: `validate_document`, `replay`, `validate_structure` and its checks, `check_gap`, `contains`, `issue` · `blocks.rs`: `validate_block` and its checks, `validate_container_gaps`, `validate_inline`, `validate_table`, `table_gap` · `sections.rs`: `validate_sections`, `validate_extractor` |

### D3 Functions over the limits

| Function | Now | Change |
| --- | --- | --- |
| `chunk_split::Layout::context_entries` | 181 lines, complexity 21 | Extract `heading_chain`, `heading_entries`, `ancestor_entries` (with `list_item_units`, `task_marker_units`, `definition_term`) and `table_header_entry`; the retain and sort stay |
| `chunks::build_batch` | 102 lines | Extract `chunk_id` and `prepared_identity` into `identity.rs` |
| `chunks::validate_chunks` | 112 lines | Extract `check_separator`, `replay_part` (returns the part's primary contributions) and `check_fragment_order` |
| `validate::validate_structure` | 117 lines | Extract `check_reference`, `check_source_ledger`, `check_usable`, `check_artifacts`, `check_uncovered` |
| `validate::validate_block` | 123 lines | Extract `check_children`, `check_parent_reference`, `check_assets`, `check_attributes` |
| `dedup::insert_group` | six parameters | Take the map key `(Representation, String)` as one parameter |
| Six test functions (`properties.rs:18`, `chunk_native.rs:92`, `dedup_contract.rs:342`, `cli_contract.rs:237`, the mapping test at `chunk_mapping.rs:503`, the chunks test at `chunks.rs:761`) | Over 100 lines or complexity 15 | Each asserted scenario becomes a helper or its own test; no assertion is removed or weakened |

### D4 Repository policies as tests

`crates/maestro-conventions/tests/policies.rs`, run by CI through `cargo test`:

| Test | Refuses |
| --- | --- |
| `rust_files_stay_within_500_counted_lines` | A tracked `.rs` file with more than 500 lines that are neither blank nor `//` comments |
| `no_personal_path_in_repository_text` | A name followed by a separator under `/home/`, `/Users/` or `C:\Users\`, in any text file outside `.git`, `.tools`, `target` and `mutants.out*` |
| `no_private_registry_or_quality_service_setting` | `ARTIFACTORY`, `JFROG` or `SONAR_` in a tracked file other than Markdown (documents may name what was dropped) |
| `every_member_inherits_the_workspace_lints` | A member manifest without `[lints] workspace = true`, or with its own `[lints.*]` table |
| `every_relative_link_and_anchor_resolves` | A Markdown link to a missing file or heading anchor (GitHub slugs, code ignored; Markdown under `examples/` is test input and skipped) |

### D5 Toolbelt and local gate

- `mise.toml`, `mise.lock`, `scripts/bootstrap.sh` follow rust-workflows at
  `8a55c53`: the tools `just check` runs, at the versions its CI pins, locked for
  `linux-x64`.
- `justfile`: `help`, `setup` (as rust-workflows), `check` (the local gate),
  `docs` (strict rustdoc), `native` (the explicit native tests) and `mutants`
  (the full local mutation run).

### D6 CI

`ci.yml` calls rust-workflows v1.2.1
(`3495c8391831b9f43b6b94f3b917ceaeb3cf64e3`) as job `rust` with
`coverage-threshold: 90`, `clippy-level: pedantic`, `license-policy: enforce`,
`artifact-key: maestro-core`, then `upload-sarif.yml` and `upload-coverage.yml`
as the organization's template does. `scorecard.yml` comes from the
organization's template; `dependabot.yml` covers Cargo and Actions weekly;
`dependabot-auto-merge.yml` is rust-workflows' copy; CODEOWNERS names the
maintainer.

### D7 Mutation testing on the import

The import pull request's diff is the whole crate: 810 mutants, which the CI
job's 45 minutes cannot hold. `just mutants` runs them all locally first, and
survivors are killed by new tests (an equivalent mutant is excluded in
`.cargo/mutants.toml` with its reason, each one reviewed). That pull request
alone sets `mutation-test: false`; the next maestro-core pull request removes
the line, and its own diff mutates in seconds.

### D8 Repositories and pull requests

| Order | Repository | Pull request | Gate |
| --- | --- | --- | --- |
| 1 | `maestro-core` | `feat: start maestro-core from the canonicalization crate` | `rust / Required Rust CI`, CodeQL, signed squash |
| 2 | `maestro-model-router` | Done on 2026-09-24 (#1); S0 checks it | The same |
| 3 | `ctm-collection` | `docs: add the private collection sources` | Review only (private, outside the organization) |
| 4 (next session) | `maestro-core` | `ci: mutation-test pull requests again` | The same as 1 |

One pull request per repository per session; the maestro-core import can take
several sessions of local commits before it is opened.

## Complexity Tracking

| Violation | Why needed | Simpler alternative rejected because | Owner | Expiry |
| --- | --- | --- | --- | --- |
| `#[expect(clippy::struct_excessive_bools)]` on `ParserOptions` | Its eight booleans are independent parser switches, serialized and included in identities | An enum set or bit flags would change the public, serialized API and every identity | Maintainer | While schema `1.2.0` holds |

## Risks

| Risk | Response |
| --- | --- |
| Surviving mutants take long to kill | Budget 0.5 to 2 days inside S0's 4 to 6; each survivor becomes a test named after the behaviour it proves |
| The relocation test needs the 634 MB model twice | Hard links on the same file system (the test's temporary directory lives under `target/`) |
| `specify init` changes its layout between versions | Scaffold in a temporary directory with the pinned 1.0.1 and copy only missing files |
| The private collection's location is still open | Default: a private repository on the maintainer's account ([08 §17](../../docs/architecture/08-traceability.md#17-open-decisions)) |
