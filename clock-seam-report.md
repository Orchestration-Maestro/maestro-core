# Clock seam report

Status: DONE_WITH_CONCERNS
Branch: `fix/s1-clock-seam`
Base: rebased onto `origin/feat/s1-integration` at `1701eab`

## Change

Added the kernel `Clock` port and `SystemClock`; `ReadControl` now carries its clock and kernel controlled reads consult it for timeout checks, SQLite progress callbacks, and remaining busy timeout. Added knowledge's captured-handle `RuntimeClock`, which enters the captured runtime before reading Tokio time. Blocking deadlines now use `deadline.into_std()`; removed `std_deadline`.

Converted admission, candidate loading/enrichment, identifier and structured retrieval, evidence assembly/ledger/selection, and source loading to use the carried clock. Source-load worker threads use the clock stored in `ReadControl`, not an ambient Tokio clock. Documentation and a normal-runtime `RuntimeClock`/`SystemClock` comparison cover the unchanged production-clock behavior.

## Proofs

- Red: polish-5a's actual 1500 ms and 700 ms assembly-stall failures are preserved at `/tmp/clockseam-reports/red-dense-stall.log` and `/tmp/clockseam-reports/red-reserve-stall.log` (original source logs: `/tmp/rev-p5a-logs/assembly-delay2.log` and `assembly-delay.log`).
- Green: inserted a temporary 1500 ms `thread::sleep` at `assemble_blocking` entry; `a_dense_query_outlasting_the_budget_is_dropped_within_the_deadline` passed. Repeated at 700 ms; `evidence_assembly_keeps_its_reserve_behind_a_hanging_route_and_rerank` passed. Scratch logs: `/tmp/clockseam-reports/green-dense-stall.log`, `/tmp/clockseam-reports/green-reserve-stall.log`. The stall was removed before validation/commit.
- Hand-mutant: temporarily changed `RuntimeClock::now` to use the system clock. `blocking_clock_control_ignores_a_real_stall_but_observes_expiry` failed at the pre-expiry assertion, as expected. Log: `/tmp/clockseam-reports/fixed-clock-mutant.log`. No `cargo mutants` run was made, following the workspace rule.
- Stage list: admission, candidates, identifier, structured, assembly, and source load use the runtime clock. The two explicit real-stall integration proofs exercise assembly; stage-specific injected-stall tests for the other workers were not added. Their control checks use `ReadControl::now()` and the shared paused-clock test covers the clock semantics.

## Gates

- `cargo fmt --all -- --check`: passed.
- `capped cargo test --workspace --locked`: passed; final captured output is `/tmp/clockseam-reports/workspace-tests-final.log` (2719 passed, 19 ignored, no failures; includes doctests).
- Org-configured workspace Clippy passed on Linux, Windows GNU, and macOS. Logs: `/tmp/clockseam-reports/clippy-linux.log`, `clippy-windows.log`, `clippy-macos.log`.
- Both strict rustdoc passes passed; logs: `/tmp/clockseam-reports/rustdoc-public-final.log`, `rustdoc-private-final.log`.
- `rust-gate guide`, duplication, and licenses passed. License output: advisories, bans, licenses, sources all OK.
- Windows Clippy required the approved host-compiler overrides for ring/SQLite; macOS used `~/.local/bin/cc-darwin`.

## Files changed

Kernel port/control and fixtures: `crates/maestro-kernel/src/retrieval/{types.rs,mod.rs,read.rs,inventory.rs,tests/deadlines.rs,tests/identifier_scope.rs,tests/identifiers.rs,tests/inventory.rs,tests/projection_storage.rs,tests/versions.rs}`.

Knowledge adapter/stages and control fixtures: `crates/maestro-knowledge/src/search/{deadline.rs,admission.rs,candidates.rs,candidate_enrichment.rs,routes/identifier.rs,routes/structured.rs,evidence/assemble/deadline.rs,evidence/assemble/engine.rs,evidence/assemble/ledger.rs,evidence/selection/types.rs,evidence/source.rs,evidence/tests/assembly.rs,evidence/tests/selection.rs,evidence/tests/source.rs,evidence/tests/support.rs,tests/candidate_enrichment.rs,tests/deadlines.rs,tests/handoff.rs}` and the two integration fixture files `crates/maestro-knowledge/tests/it/qdrant_projection/search_routes/search_projection.rs`, `crates/maestro-knowledge/tests/it/synthetic_gate/search.rs`.

Open risk: the brief requested direct stall proofs for every converted worker; only the shared paused-clock contract and two assembly worker stalls are directly exercised. The other stages are converted and covered by the full suite but do not have individual injected-stall tests.
