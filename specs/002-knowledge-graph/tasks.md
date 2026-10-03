# Knowledge Graph Implementation Tasks

> **For agentic workers:** use superpowers:executing-plans for the assigned
> task. The supervisor owns dispatch, review and integration; a lane starts
> no subagents. Checkboxes record integrated evidence, not worker claims.

**Goal:** M2, relationships answered with complete source proofs and measured
quality, starting with a rule-only neighbors pilot.

**Architecture:** SQLite/artifacts own claims; qualified embedded LadybugDB
projects them. Reuse S1 jobs, generations, evidence, eval, CLI and MCP. One
engine, no service or runtime SQL fallback. Deterministic claim-first descriptors
use the existing Qdrant infrastructure; a separate dependent passage route uses
source transitions, never transition edges as relationship proofs.

**Tech Stack:** Rust 1.98.1 (MSRV 1.98), existing workspace dependencies and
owner-approved `lbug` with the pin/features qualified by G25.

**Input / prerequisites:** [spec.md](spec.md), [plan.md](plan.md), the approved
D1–D8 decisions and the integrated S1 seams. Research evidence comes from G25;
it is not a prerequisite to writing these documents or a result assumed here.

## Format and path conventions

Stable IDs preserve the draft's references; G29's second loader is removed.
**G25 is first**; public construction starts beside it, but pilot reads wait.
Tasks are dependency-ordered, not numeric. There are **37 stable G tasks**:
35 non-umbrella initial budgets total **132 lane-hours**, each at most four
hours including targeted gates, plus **9 h** of reopened follow-ups and **3 h**
of other cross-slice G follow-ups (**144 h**). G28's **four ordered slices**
total **13 h**, including its original 1 h cross-slice allocation, for
**157 bounded lane-hours**. G27's **14 E slices** total **43–89 h**, including
their **3 h** cross-slice delta, for **200–246 lane-hours** overall. The old
G27 4 h and G28 5 h umbrella budgets are excluded, not counted twice.
The 2026-10-03 16:38 Oracle budget ruling adds 8 h: the old G28 estimate
predates the resolution pin, durable binding and explicit resume obligations
surfaced in implementation. Scope and acceptance do not change. These are
planning budgets, not observed durations. Slice and DAG accounting are below.
Every task has a failing check, exact file targets and acceptance criteria.
Split an overrun before dispatch rather than omit a check. Native CI waits are
reported separately; a four-hour timebox does not turn pending CI into a pass.
`[P]` after the story tag marks a task eligible for a parallel lane after its
inputs land. It does not authorize concurrent edits: G09 precedes G10; other
migration/CLI registration hunks are supervisor-rebased exceptions, and all
remaining shared files have serialized ownership.

Paths below use these exact directory bindings, with no wildcard ownership:

| Binding | Directory |
| --- | --- |
| `K` | `crates/maestro-kernel` |
| `N` | `crates/maestro-knowledge` |
| `C` | `crates/maestro` |
| `G` | `crates/maestro-knowledge/src/graph` |
| `P` | `crates/maestro-knowledge/src/graph/projection` |
| `S2` | `specs/002-knowledge-graph` |
| `PRIVATE` | Owner-bound private collection checkout, never a public path |

A listed path not in the baseline is created by that task. Existing paths are
extended. New modules also register in their parent's `mod.rs` or `lib.rs`;
child `mod.rs` files contain only declarations and re-exports. Integration
modules under `C/tests/it/` and `N/tests/it/` register in their existing
`main.rs`; do not add extra lbug-linked test binaries. These registrations and
generated guide updates are shared-file edits, not wider scope. Per the
2026-10-03 15:16 migration-number ruling in ledger
`s1-knowledge-kernel/progress.md`, new migrations use S2 **0030–0039**,
S3 **0040–0049**, S6 **0050–0059**. G28b uses `0030_graph_input_pins`;
the slice merging to main second renumbers the existing 0019 collision.
Historical `NNNN` targets below refer to already assigned names in the
[plan's migration table](plan.md#migration-landing-order), not new reservations.
Timestamps in this amendment cite that same ledger.

## Reopened by the 2026-09-30 amendment

These are new obligations on existing task IDs, not credit for their landed
implementations. The supervisor dispatches one follow-up lane per row, using
that task's existing Files/Test/Acceptance and the delta below. Each bounded
estimate includes targeted gates; calibration runtime, GPU admission and human
review waits remain separate. If a delta cannot fit, split before dispatch.

| Task | Delta | Bounded hours | Dispatch trigger |
| --- | --- | ---: | --- |
| G06 | Golden 84/16 answer/refusal gates, power-table reproduction, 2,880-request completeness and the single scorer's proof-attrition/citation-support diagnostics. | 3 h | Landed scorer available; dispatch after this amendment, before G07/G08 freeze or G38 measurement. |
| G10 | Same-name documentary namespace audit and refusal/review tests; after the audit, recheck G35's descriptor linking and projection handoff against the audited identities and fix findings in this same follow-up; no silent identity-key change. | 2 h | Landed resolver and G31 follow-up available; required before G35 linking/projection and G38 selection, with the landed-G35 exception recorded below. |
| G18 | Explicit instance `Job` and event-satisfaction-as-dependency refusal tests on the constrained gateway. | 1 h | Landed extraction gateway available; before G19 calibration and G38 inference. |
| G31 | Matching `Job`/event refusal cases at the closed claim-authority boundary. | 1 h | Landed vocabulary available; before G10 audit and downstream G11/G19/G27 qualification. |
| G19 | Fixed-window timed calibration with token/wall/GPU/rejection/retry receipts and documentary-scope checks. | 2 h | Landed extractor plus G10/G18/G31 follow-ups and G38 step 0's pre-use card receipt; approved development receipt and GPU slot before timing; before G38 selection. |

Total incremental budget: **9 h**, additional to those tasks' initial
allocations, not a replacement or claim of remaining-project effort. Reopened
tasks retain their requirement ownership. For the conservative full-plan DAG below, add each row's
hours to its named task and require that follow-up before its downstream
consumers; an already landed base never satisfies the amended completion gate.

**Recorded ordering exception:** G35 landed as `bc5f722` + `cfcf788` + `f043e30`
before G10's reopened namespace audit, contrary to the dispatch trigger above.
Those commits are integrated descriptor work, not evidence that the audit ran.
G10 retains its **2 h** follow-up: audit, recheck the landed linking/projection
handoff against the audited identities, and fix anything found in that same
follow-up. No silent identity-key change is permitted. Keep G10 → G35 as the
required completion order; the historical exception adds no reverse G35 → G10
edge and does not mark either amended obligation complete.

## Approved manifest v4 cross-slice additions

The owner-approved S3 `30b702b` handoff (`specs/003-catalog/tasks.md:3969–3974`)
adds these deltas to existing IDs, not new tasks or completion credit. Preserve
each task's Files, Test and Acceptance; use them for the delta's failing-first
checks too. G follow-ups are separately dispatched, so a historical 4 h task
plus its 1 h delta is not permission for a 5 h lane.

| Task | Delta | Additional hours | After |
| --- | --- | ---: | --- |
| G01 | Reconcile S2's graph contract with C44's approved manifest v4 backend/settings contract. | 1 h | C44; retain the decided D3 and integrated S1 inputs. |
| G26 | Consume C46's registry-backed graph settings through existing setup/health; preserve disabled/unavailable and owned-root boundaries. | 1 h | C46, G25, G01. |
| G27 E07a | Wire C47a's frozen backend settings and complete non-resource lock handoff into the qualified native adapter. | 2 h | C47a, G01, combined E01/E02/E03/E03b fork, G25, released E04. |
| E08b | Carry the approved backend settings and lock handoff through reader, writer and publication paths. | 1 h | E08a, E06, unchanged; reader/writer/publication names the delta's scope, not new predecessors. |
| G28 | G28b owns durable resolution/settings/lock pins and open/probe comparison; G28c/G28d reuse those pins for resume/rebuild, never changed-input replay or latest-resolution lookup (2026-10-03 12:08, 13:36, 15:30–15:58). | Original 1 h is inside the replacement 13 h slice budget, not additional. | C47a, G26, G35 and G27's qualified native lifecycle; E11 is not a G28 start gate (13:33). |
| G22 | Use C48's actual qualified backend declarations with the existing release/drill inputs; provide native/release evidence to C49a. | 1 h | C48, G14, G15, G16, G19, G30; retain release inputs. |

The original increment is **7 h**: **4 h** G work (including G28's 1 h, now
inside its 13 h slice budget) and **3 h** already included in the E07a/E08b
ranges below. Do not add either included allocation twice. Landed G01/G26
base work does not satisfy these additions. Qualified pins are execution
inputs from observed qualification, never invented by this amendment.
The release order is **qualification → C48 → G22 → C49a**, never C48 ↔ G22;
C49a additionally requires **E11 and G28**, alongside its S3 prerequisites.

## S2 integration workflow and global gates

Use a fresh clone from `origin/feat/s2-integration`, one signed conventional
commit per task and only the assigned lane branch. Never push the integration
branch or open an individual task PR to main. The supervisor reviews and
fast-forwards accepted work. Documentation here does not mark M1 delivered.

1. Write the **Red** cases and run the task's **Test** command. Save the failing
   output; for a research/private task, first demonstrate that the missing or
   invalid evidence is rejected. Synthetic evidence is not a real receipt.
2. Implement only the **Green** deliverable. Run focused tests and host Clippy
   while editing; no model, engine or network I/O inside a SQLite transaction.
3. Run the **Check** cases and local gates, then commit with hooks enabled.
   Push that tested signed commit with `git push --no-verify` to the lane branch.
   Report its hash, wall time, failures and gate output.
4. Full coverage, native three-OS builds/tests and mutations run on stable
   GitHub CI, not locally. Before S2 merges: at least 95% changed-line / 90%
   total coverage, zero missed mutants and zero timeouts. Review and the
   supervisor's integration check still gate a landing.

Local build environment (every Cargo command runs through the memory cap):

```sh
export PATH="$HOME/.local/bin:$HOME/.cache/maestro/tools/bin:$PATH"
export CARGO_BUILD_JOBS=3 NEXTEST_TEST_THREADS=24
export CARGO_PROFILE_DEV_DEBUG=line-tables-only
export CARGO_PROFILE_TEST_DEBUG=line-tables-only
export CLIPPY_CONF_DIR="$HOME/.cache/maestro/org-clippy"
```

Before a code push: `capped cargo fmt --all --check`, host
`capped cargo clippy --workspace --all-targets --locked -- -D warnings`, and
`capped cargo nextest run --workspace --locked`. Run cross-target Clippy with
G25's approved native recipe; existing bundled SQLite/ring flags alone do not
qualify lbug. The mandatory target commands are:

```sh
capped cargo clippy --workspace --all-targets --locked \
  --target x86_64-pc-windows-gnu -- -D warnings
capped cargo clippy --workspace --all-targets --locked \
  --target aarch64-apple-darwin -- -D warnings
```

Also run the installed commit hooks (Markdown, offline links, line length,
spelling, secrets, hygiene, rules and guide), `rust-gate architecture --local`,
`rust-gate duplication` and `rust-gate licenses` with the lane rule recipes.
A changed lock also requires `capped cargo vet --locked`. Dependencies belong
in root `[workspace.dependencies]`; exceptions belong in
`maestro-quality.toml`, never a hand-edited generated policy. No local full
`just check` or mutation run. A documentation-only task runs the document and
repository hooks, conventions tests, conventions-crate Clippy on all three
targets, architecture, duplication and licences, not unrelated Rust builds.

All public examples and process fixtures are synthetic. Keep vendor rules,
quotes, questions, prompts and reports private; use the approved local model
profile. In private tasks, lanes run commands exposing only aggregate metrics,
IDs and digests; no private text enters a hosted model context. Extraction and
independent review run locally through the router; the reviewer is a different
local model family from its catalog, and the owner rules on flags on a local
review page (plan Validation). No automatic egress, new version or unapproved
dependency. A search, answer or extraction call uses `Room::Free` and never
evicts a chat model.

## Phase 1: Qualification gate

### G25 [US5] [P] Qualify lbug before adopting it

**Time:** 4 h. **After:** none; the separate qualification lane owns this work.
**Files:** `N/tests/it/lbug_qualification.rs`, `S2/research.md`, root
`Cargo.toml`, `N/Cargo.toml`, `Cargo.lock`, `maestro-quality.toml`,
`supply-chain/config.toml`, `supply-chain/audits.toml`,
`.github/workflows/lbug-qualification.yml` (in maestro-core).
**Test:** `capped cargo nextest run -p maestro-knowledge --test it lbug_qualification`.
**Acceptance:** FR-S2-001, SC-S2-007; the six-row adoption bar in plan A1,
with actual versus planned platform evidence distinguished. No weakened gate.

- [ ] **Red.** Assert types/parameters, rollback, the single batch loader,
  per-hop filtered bounded paths and native cancellation. Spawn independent
  writer/CLI/MCP reader processes; test file locks, immutable old pins, a
  second writer and kill/reopen. Unsupported behavior fails/blocks.
- [ ] **Green.** Pin minimum features and bundled native source. Check no
  native-build network/system OpenSSL, licences and at most one named forced
  duplicate. Record metadata/feature tree, clean/warm build and binary delta,
  real-cache cost per mutation shard/coverage run and peak capped memory.
  D8 approves E01/E02/E03/E03b, not adoption: qualify their combined immutable
  40-character SHA/lock pin. Test source-default out-of-repository env-unset
  builds with an empty native cache and zero trapped fetch attempts. Audit the
  full rooted native operation surface, restricted extensions/COPY and no
  ambient-path fallback. Record true-cold, fresh-target/native-cache-hit and
  warm Rust-only builds separately; a cache hit never satisfies true-cold cost.
- [ ] **Check.** Apply the six-row bar: clean CI delta ≤25 minutes, unrelated
  changes never rebuild liblbug; shards fit 30 minutes and local builds 8 GiB
  at three jobs. Linux evidence and working gate-preserving recipes for both
  cross-Clippy targets, plus native Windows/macOS evidence or a supervisor-
  approved dated CI plan, permit implementation. A missing cross-Clippy recipe
  is blocked; real native evidence still gates M2. Put the probe in core; any
  reusable toolchain release/re-pin is a supervisor step with its wait recorded.
  Publish the result in `S2/research.md` and the supervisor's G25 report, never
  infer a passed test. Dependency/lock/vet/probe edits integrate only on a
  reviewed pass; otherwise land only `research.md` and the report. Recheck the
  ID-keyed Constitution Check against the actual verdict now, recording rule
  evidence/blockers in `research.md`; G24 performs the final acceptance recheck.

**Checkpoint:** G25 gates pilot reads G04 and engine tasks G11, G21, G22,
G26, G27, G28 and G30. Public claims/rules and model-role groundwork can
start beside it. No task waits for M1 release; SQLite never answers graph queries.

## Phase 2: User Story 1 — the rule-only pilot

### G01 [US1] [P] Freeze the pilot and reconcile the design

**Time:** 3 h initial + 1 h cross-slice follow-up (4 h total).
**After:** C44; decided D3 and the integrated S1 head remain required.
**Files:** `S2/spec.md`, `S2/plan.md`, `S2/tasks.md`,
`docs/adr/0021-embedded-ladybug-graph-projection.md`, `docs/adr/README.md`,
`docs/adr/0004-neo4j-for-the-graph-projection.md`,
`docs/adr/0020-rust-libraries-with-named-dependency-exceptions.md`,
`docs/architecture/README.md`, `docs/architecture/01-knowledge-pipeline.md`,
`docs/architecture/02-retrieval-and-knowledge-graph.md`,
`docs/architecture/04-intelligence-backend.md`,
`docs/architecture/05-platform-and-operations.md`,
`docs/architecture/06-roadmap.md`, `docs/architecture/08-traceability.md`,
`tests/fixtures/synthetic/graph/defaults.md`,
`tests/fixtures/synthetic/graph/defaults.json`, `N/tests/it/graph_fixture.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge --test it graph_fixture`.
**Acceptance:** FR-S2-004, FR-S2-021, SC-S2-001; one synthetic table fixture,
reviewed requirements and a private scope receipt reference, no vendor text.

- [x] **Red.** Fixture checks reject mismatched digest, UTF-8 spans and
  defaults; check the requirement table equals the Acceptance lines exactly.
  Inventory ADR-0020's neo4rs wording, architecture 08's open Neo4j-first row,
  architecture 06's platform deferral until after S4, and architecture 05's
  Neo4j service row, the graph design in 02, README's graph stack, 01's
  entity-vector row and 04's B6/D02 graph references. For the v4 delta, reject
  drift from D14's named controls/bounds, missing registry/lock handoff and
  retired engine resources in G01's documents and synthetic fixture.
- [x] **Green.** Reconcile those rows with embedded-first S2, later selected
  Neo4j, three-OS gates and the minimal typed-edge seam. Pin one table rule
  and synthetic fixture; keep private receipt content private. Allocate no
  migration numbers here. G24, not this task, finalizes ADR qualification
  against G25. Do not call the pilot M2. Reuse the v4 names/shapes below;
  keep the extraction fixture's source/rule/oracle digests unchanged.
- [x] **Check.** Fixture and document hooks pass; architecture rows distinguish
  planned work from delivered evidence and name deferred graph algorithms.
  Verify existing search/ask, backup, scopes and generation seams at dispatch.
  For this delta run the Test command and
  `prek run --from-ref origin/feat/s2-integration --to-ref HEAD`.

**Manifest delta contract:** the [plan handoff](plan.md#manifest-v4-settings-and-lock-handoff)
is sourced from S3 `30b702b` D13/D14 and C44/C46/C47a/C48. Keep `graph.engine`
as the setting, with backend `type = "ladybug"` translated by the registered
adapter from `core/backends/graphdb/config.toml`. C46's core backend defaults
and `settings/defaults.toml` feed one lowest registry slot, one producer per
key; only `graphdb.buffer_pool_size`, `graphdb.max_db_size` and
`graphdb.max_num_threads` are the approved tuning keys. Keep D14's bounds,
whole-file/inactive-table validation, masked-invalid and locked refusals,
zero-call `none` and unavailable/owned-root boundaries. C47a freezes admitted
defaults and the complete non-resource lock; bundles preserve checked configs,
and changed config/lock cannot replay. C48 supplies qualified declarations,
not guessed pins; approved owner identities/protections remain **OA1**.
C46 migration/encoded-value details and C47a's concrete wire shape are
implementation inputs not specified by S3, not new approval IDs. Preserve D3
and integrated S1 inputs. G26, E07a, E08b, G28 and G22 deltas remain open;
this document/fixture check provides no native, private or release evidence.

G01's frozen public rule/oracle and dispatch inspection are in plan A0 and
Starting point. The private reference `PRIVATE/graph/receipts/pilot-inputs.json`
remains unverified here; no S1 receipt applies. The owner approved the pilot
and acceptance scopes at 12:53. G05 still needs its recorded 9.0.22-table receipt;
G07 needs separate `PRIVATE/graph/receipts/acceptance-inputs.json` for the whole
published generation, inventory and window policy, processed locally. The `graph_fixture` test checks this
fixture and the requirement map below; it does not implement G02–G04 or prove
private pilot success. Checkboxes remain for supervisor integration evidence.

### G02 [US1] [P] Store verified immutable claims

**Time:** 4 h. **After:** G01.
**Files:** `K/src/facts/types.rs`, `K/src/facts/write.rs`,
`K/src/facts/error.rs`, `K/src/facts/read.rs`, `K/src/facts/quote.rs`,
`K/src/facts/tests/claims.rs`, `K/src/facts/tests/support.rs`,
`K/src/facts/tests/supports.rs`, `K/src/facts/tests/schema.rs`,
`K/migrations/0012_graph_claims.sql`, `K/src/store/migration.rs`,
`K/src/store/tests/migrations.rs`.
**Test:** `capped cargo nextest run -p maestro-kernel facts`.
**Acceptance:** FR-S2-002, FR-S2-003; scoped literal `DEFAULTS_TO` claims,
conditions/time/profile/review fields and immutable verified supports.

Landed at `a176f26`: kinds are nonempty strings and objects are literals only.
G31, not this historical task, adds the closed kinds and entity-valued claims.
Quote checks live in `facts/quote.rs`; moving them under evidence would create
an import cycle. G04 reuses these authority reads, not a graph query in SQLite.

- [x] **Red.** Reject missing/empty anchors, wrong revision/digest, out-of-range
  or split-UTF-8 spans, denied grants and replacement writes. Force rollback
  after a support failure; replay the same admitted claim idempotently.
- [x] **Green.** Add the forward migration and one scoped write path that
  checks original bytes/eligibility independently of knowledge. Persist frozen
  membership and provenance; expose typed claim/support records for G03, not
  unscoped SQL or canonicalization types.
- [x] **Check.** Run the Test command plus
  `capped cargo nextest run -p maestro-kernel store::tests::migrations`.
  Failed writes leave no accepted relation, fresh/reopened stores agree, and
  valid quotes never automatically promote semantic truth/review state.

### G03 [US1] [P] Extract the first table rule

**Time:** 4 h. **After:** G02.
**Files:** `G/rules.rs`, `G/verify.rs`, `G/structure.rs`,
`G/tests/rules.rs`, `G/tests/verify.rs`, `G/tests/support.rs`,
`K/src/facts/types.rs`, `C/src/cli/graph/build.rs`,
`C/src/cli/args.rs`, `C/src/cli/run.rs`, `C/tests/it/graph_build.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph` and
`capped cargo nextest run -p maestro graph_build`.
**Acceptance:** FR-S2-003, FR-S2-004; synthetic import → rule build produces
only correctly located `DEFAULTS_TO` claims through G02.

Integrated as `79b6502` on top of `a176f26`: `rules.rs` exposes the `Extractor`
port; its `resolve` groups subjects across documents and reports spelling/kind
collisions.
G10 preserves and extends that resolver; it does not introduce a second one.

- [x] **Red.** Refuse duplicate/unknown keys, changed digests, executable
  fields, misleading headings and invalid/ambiguous quotes. Keep G03's existing
  cross-document identity/collision tests as the baseline G10 extends. Assert
  `DEFAULTS_TO` has a typed literal object and creates no literal/Document/
  Section nodes.
- [x] **Green.** Parse the standalone closed `rule` object defined in plan A0:
  `id`, its bound `source_sha256` and explicit subject/type/lexeme columns;
  never accept the test envelope or its `expected` oracle as rule input. Build
  the temporary import declaration/manifest from A0's fixed metadata, not S1's
  corpus. Reuse block/source references and submit candidates through G02.
  Preserve literal types and source lexemes, not floating-point rewrites or
  invented entities. Carry source references without projecting structural
  nodes; retain rejections.
- [x] **Check.** Run both Test commands; valid Unicode source slices agree
  byte-for-byte with the original. A rule/profile change changes the frozen
  inputs. The build runs without a model, script or graph service.

### G06 [US4] [P] Score construction, complete proofs and answers

**Time:** 4 h. **After:** G01, G03.
**Files:** `N/src/eval/graph/labels.rs`, `N/src/eval/graph/score.rs`,
`N/src/eval/graph/gates.rs`, `N/src/eval/graph/tests.rs`, `N/src/eval/ladder.rs`,
`N/src/eval/bootstrap.rs`, `C/src/cli/eval/graph.rs`,
`C/src/cli/eval/runner.rs`, `C/src/cli/eval/manifest.rs`,
`C/src/cli/eval/command.rs`, `C/src/cli/eval/graph_output.rs`,
`C/src/cli/eval/tests/reports.rs`, `C/src/cli/args.rs`, `C/src/cli/run.rs`,
`tests/fixtures/synthetic/graph/eval.jsonl`, `C/tests/it/graph_eval.rs`,
`S2/research.md`.
**Test:** `capped cargo nextest run -p maestro-knowledge eval::graph` and
`capped cargo nextest run -p maestro graph_eval`.
**Acceptance:** FR-S2-013, FR-S2-015, FR-S2-018, SC-S2-002, SC-S2-003, SC-S2-004; digest-bound
labels and hand-computed stage scores, using S1 statistics rather than a second
runner. Add only `eval graph check`; runs/comparisons use `eval ladder`.

- [ ] **Red.** Partial chains score zero, a complete alternative once. A proof
  lost to evidence/wire budgets scores zero: every required anchor must survive
  in the delivered evidence bundle under the shared context budget. Reject
  duplicate families, false exact quotes, missing reviews and digest mismatch.
  Assert `eval graph check` never opens the default kernel; missing/overlapping
  scratch bindings refuse before any open, with a default-kernel open spy.
  Invalid questions, labels, quotes and nested error sources never appear on
  standard output or standard error: only fixed error codes, item IDs and digests.
  Test the private ladder path too, not only graph check. G32/G33/G34 reuse
  these refusal/output checks for review, owner pages and drafting.
  With 200 independent answerable items, seed-0/2,000 paired resamples accept
  10 wins/0 losses at five points and reject 9 wins; assert 50th/1,950th bounds.
  Keep N=80 power fixtures separately: 4/0 passes, 5/1, 6/2 and 8/4 fail.
  Golden 84-answerable support and all-16 refusal regressions fail separately;
  errors/timeouts receive no credit and missing answers cannot become a
  retrieval-only pass. Reject any run missing one of the 2,880 required requests.
- [ ] **Green.** Add strict labels, one construction scorer and graph metrics
  to the existing ladder. `eval graph check --manifest` validates without
  inference; existing ladder rungs run and compare, keeping every attempt.
  Own shared `graph_output` emission for private eval success/errors, including
  existing ladder entry points; raw diagnostics go only to approved private
  files. Implement the spec's gate-rule table, not a second ladder runner.
- [ ] **Check.** Run both Test commands. Assert same-run passage-only comparison
  (S1 default routes/weights/reranker, graph `none`), all-three-runs pass logic,
  95% precision point estimate, all-unanswerable refusal denominator and warm
  cohort limits. `ctm-retrieval` Recall@10/MRR@10 deltas below zero fail each run;
  a nonnegative delta passes even if the diagnostic seeded interval crosses zero.
  Historical G08 drift and route/pre-fusion recall are separate diagnostics.
  Before G07/G08 freeze, record D7's 200+20 count and reproduce plan A6's full
  power table (5,000 synthetic datasets per cell; S1 inner generator, unchanged
  joint gate, outer PCG64 seeds and rates as specified there). Save parameters,
  counts and results in `S2/research.md`; the table is simulation, not corpus
  performance. Disclose the exact-5-pp boundary and dependent-repeat limits.
  G06 owns candidate/pre-fusion/post-packing/final-wire proof-attrition,
  citation-support coverage and unsupported-conclusion scoring, with explicit
  denominators and fixtures distinguishing present citations from supported
  conclusions. G13/G16 supply observations; G23 reports this single scorer's
  output, never a second implementation. The reopened budget above owns these
  changes; landed scorer evidence alone does not complete the amendment.

### G34 [US4] [P] Draft private questions with a bounded local runner

**Time:** 4 h. **After:** G06.
**Files:** `N/src/eval/graph/draft.rs`, `N/src/eval/graph/tests/draft.rs`,
`C/src/cli/eval/graph_draft.rs`, `C/src/cli/eval/manifest.rs`,
`C/src/cli/args.rs`, `C/src/cli/run.rs`, `C/tests/it/graph_draft.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge eval::graph` and
`capped cargo nextest run -p maestro graph_draft`.
**Acceptance:** FR-S2-018, FR-S2-021; locally drafted, source-anchored private
questions remain unreviewed until the independent reviewer and owner act.

- [x] **Red.** Refuse missing/expired scope, overlapping scratch bindings,
  nonlocal/unpinned cards, insufficient room, invented anchors, budget/output
  overflow and duplicate families. Question/label/quote text and nested errors
  never reach stdout/stderr; replay does not duplicate draft items.
- [x] **Green.** Add `maestro eval graph draft --manifest` over the existing
  local gateway with `Room::Free`, pinned card/prompt/input digests and bounded
  per-window/token work. Read only the approved acceptance inventory/windows;
  write draft labels, provenance and every failed attempt directly under
  PRIVATE, without treating model text as verified source evidence. Reuse
  G06's label checker and safe aggregate/ID/digest output; add no second ladder.
- [x] **Check.** Run both Test commands with synthetic sources and fake local
  inference. Revalidate anchors against the approved inputs before saving a
  candidate; keep its unreviewed state. G07 executes this runner; G08 reviews
  all its outputs with G32 and obtains flagged rulings through G33.

### G31 [US2, US4] [P] Admit entity-to-entity claims from the closed vocabulary

**Time:** 4 h. **After:** G02, G03.
**Files:** `K/src/facts/types.rs`, `K/src/facts/write.rs`,
`K/src/facts/read.rs`, `K/src/facts/error.rs`, `K/src/facts/tests/claims.rs`,
`K/migrations/NNNN_graph_claim_vocabulary.sql`, `K/src/store/migration.rs`,
`K/src/store/tests/migrations.rs`, `G/rules.rs`, `G/tests/rules.rs`.
**Test:** `capped cargo nextest run -p maestro-kernel facts` and
`capped cargo nextest run -p maestro-kernel store::tests::migrations`.
**Acceptance:** FR-S2-002, FR-S2-003, FR-S2-024; closed subject/object kinds and
predicates, entity or literal endpoints with exact existing support checks.

- [x] **Red.** Reject kinds/predicates outside architecture 02 §8.2, `ALIAS_OF`
  claims, `DEFAULTS_TO` with an entity object and entity predicates with a
  literal object. Reject unauthorized object endpoints. Upgrade a populated
  0012 database; valid rows, claim/set digests, supports and history survive.
  An out-of-vocabulary legacy row refuses the whole upgrade with a clear
  ID-only diagnostic and no database change; invent no mapping or new claim.
  Test documentary job types/events without an instance `Job` kind; event
  satisfaction must not be coerced into a dependency. C27a remains separate.
- [x] **Green.** Add typed subject/object kinds and entity-to-entity predicates
  through the existing authoritative write/read paths. Rebuild the claims
  table in the next-free forward migration, preserving immutability, scope,
  frozen membership and old valid literal identities. Adapt G03's literal
  construction to the typed API without changing its source bytes or digests.
  SQLite remains authority only, never a neighbor/path implementation.
- [x] **Check.** Run both Test commands and
  `capped cargo nextest run -p maestro-knowledge graph`. Reopen upgraded stores,
  verify identical valid legacy records and round-trip supported entity claims.
  G11, G19 and G27 cannot start before this contract lands.

### G09 [US1, US2] [P] Make builds resumable and attachments immutable

**Time:** 4 h. **After:** G03.
**Files:** `G/build/run.rs`, `G/build/checkpoint.rs`, `G/build/tests.rs`,
`K/src/facts/build.rs`, `K/src/facts/tests/builds.rs`,
`K/migrations/NNNN_graph_builds.sql`, `K/src/store/migration.rs`,
`K/src/store/tests/migrations.rs`, `C/src/cli/graph/build.rs`,
`C/tests/it/graph_resume.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::build` and
`capped cargo nextest run -p maestro graph_resume`.
**Acceptance:** FR-S2-005; one frozen claim/profile attachment per generation,
leased checkpoints and bounded durable acceptance/rejection receipts.

- [x] **Red.** Kill before/after checkpoint commit, expire/take over a lease,
  race two workers, change a profile and retry an accepted batch. Partial
  builds stay invisible and old pins cannot observe new claims.
- [x] **Green.** Reuse kernel jobs/resources and journal progress. Separate
  extraction/verification I/O from transactional receipt writes. Attach only a
  verified frozen claim set, once; new inputs require a new generation.
- [x] **Check.** Run Test plus `capped cargo nextest run -p maestro-kernel facts`
  and `capped cargo nextest run -p maestro-kernel store::tests::migrations`.
  Resume produces the same claims/rejections as a clean build and rejects late
  writes by an expired worker. Limits survive restart rather than resetting.

### G10 [US2] Resolve sourced identities, versions and contradictions

**Time:** 4 h. **After:** G02, G03, G09, G31.
**Files:** `G/rules.rs`, `G/tests/rules.rs`, `G/resolve.rs`, `G/tests/resolve.rs`,
`K/src/facts/resolve.rs`,
`K/src/facts/tests/resolution.rs`, `K/migrations/NNNN_graph_resolution.sql`,
`K/src/store/migration.rs`, `K/src/store/tests/migrations.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::tests::resolve`
and `capped cargo nextest run -p maestro-kernel facts::tests::resolution`.
**Acceptance:** FR-S2-002, FR-S2-006; scoped sourced aliases/mentions, reviewed reversible
resolution and preserved temporal/conditional contradictions.
G05 is a later private pilot-review checkpoint, not a construction prerequisite.

- [x] **Red.** Preserve G03's `rules::resolve` identity/collision cases, then
  add sourced reversible decisions across documents and collections. Different
  colliding spellings, kinds or collections do not auto-merge. Add unrelated
  same-name documentary namespaces within one collection: unresolved collision
  blocks automatic resolution for review; a key change needs a separate ruling. Test typed literals remain claim properties with no literal/
  Document/Section nodes; also conflicting defaults, version bounds, unknown
  validity and superseded claims under old pins.
- [x] **Green.** Move and extend G03's existing resolver in `G/resolve.rs`,
  preserving its public re-export and tests; add sourced reversible review
  decisions, not a duplicate algorithm. Do not merge identities by fuzzy/vector
  similarity or turn literal values into entities. G14's descriptor candidates
  are retrieval hints, not a second resolver. Duplicate supporting copies are one group, not corroboration.
- [x] **Check.** Run both Test commands and
  `capped cargo nextest run -p maestro-kernel store::tests::migrations`;
  known bounds are half-open, record time never supplies unknown world time and
  current grants protect aliases and review records as well as claims.
  The reopened delta also rechecks G35's descriptor linking and projection
  handoff against the audited identities, using G35's descriptor tests as well
  as the existing resolver tests. Fix findings in this same follow-up; preserve
  the separate-ruling requirement for any identity-key change.

### G26 [US5] [P] Package and diagnose local embedded operation

**Time:** 3 h initial + 1 h cross-slice follow-up (4 h total).
**After:** G01, G25, C46.
**Files:** `C/src/cli/setup/graph.rs`, `C/src/cli/setup/command.rs`,
`C/src/cli/health/graph.rs`, `C/src/cli/health/doctor.rs`,
`C/src/cli/health/status.rs`, `C/tests/it/graph_operations.rs`,
`docs/how-to/knowledge-graph.md`, `S2/research.md`.
**Test:** `capped cargo nextest run -p maestro graph_operations`.
**Acceptance:** FR-S2-022; G25's actual runtime/files packaged without a graph
server, port, Docker, JVM or first-use download; health never mutates a graph.

- [ ] **Red.** Graph `none` is disabled with zero opens/probes, unlike an
  unavailable selected graph. Relocate data and simulate permissions, locks
  and corruption. Refuse caller-supplied engine paths before opening a handle.
  Cleanup refuses live readers and non-owned/authority files.
- [ ] **Green.** Extend existing setup/health with previewed owned directories
  and read-only diagnostics. Document qualified reader/writer ownership and
  offline rebuild/removal. Do not add a graph service manager.
- [ ] **Check.** Run Test with network unavailable. Record install/open/reopen
  time, binary size, RSS and disk from the qualified runtime; diagnostics name
  actionable repairs without deleting files or fetching assets.

G26 notes: `graph.engine` and the `engine` feature exist, but `maestro` links
no engine yet. Health reads graph files through `PublishedGraph` (G26's adapter
publishes none) and opens them through `PublishedFile`/`OpenGraph` with fakes in
its tests. The Red item "cleanup refuses live readers and non-owned files"
moves to G27, the first task that writes graph files: no receipt names them
before then. Measurements are in research.md.

**Durable-binding handoff (2026-10-03 12:08 ruling):** G26 compares real
admitted typed settings and the complete frozen lock between factory, producer
and active readers, without changing the persisted format. **G28b** owns
persistent build/readiness/native pins, the versioned schema bump and
receipt/native/admission comparison at open/probe. Until that lands, health's
"not yet bound" is informational, not durable verification. G28b replaces it
with bound or mismatch plus rebuild repair; G28c cannot replay changed inputs.

### G27 [US2, US5] Expose the typed-edge port and verify unpublished projection builds

**Time:** umbrella; the old 4 h is excluded. E-slice estimates below replace it.
**After:** G25, G09, G10, G31 for the complete native deliverable; independent
E05/E06 and fork/gate prerequisites can run earlier as the breakdown states.
**Files:** `P/port.rs`, `P/writer.rs`, `P/schema.rs`, `P/tests/writer.rs`,
`P/tests/port.rs`, `K/src/facts/projection.rs`,
`K/src/facts/tests/projection.rs`, `K/migrations/NNNN_graph_projection.sql`,
`K/src/store/migration.rs`, `K/src/store/tests/migrations.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::projection` and
`capped cargo nextest run -p maestro-kernel facts::tests::projection`.
**Acceptance:** FR-S2-001, FR-S2-007, FR-S2-008, FR-S2-022, FR-S2-023, SC-S2-007; public application-ID
write/read operations at `crates/maestro-knowledge/src/graph/projection/port.rs`,
edge-family separation and kernel-controlled readiness. S3 C27a depends on this
port for catalog dependency edges, which never become evidence-span claims.
The same port's `entity_facts` reads literal-valued claim records on subjects;
only entity-to-entity claims produce edges.

- [ ] **Red.** Round-trip a `DEFAULTS_TO` subject fact record with its claim
  application ID, predicate, literal type/lexeme and generation. Assert no
  literal node or edge is created; denied/unknown subjects remain identical.
  Test partial writes, wrong IDs/stamps/counts, expired leases,
  two writers and publication races. Round-trip another slice's authoritative
  typed edges without source spans; those edges must never become knowledge
  claims/proofs or cross family/scope boundaries.
- [ ] **Green.** Expose typed-edge operations and scoped `entity_facts` on
  application IDs, frozen generations and explicit families. A literal-valued
  claim is a typed fact record on its subject, keyed by claim application ID,
  with predicate, literal type/lexeme and generation; preserve the claim's
  qualifiers and support references for kernel rechecks. It is not a node,
  an edge or a truth guarantee. No engine IDs/raw Cypher cross the port. Keep lbug
  calls in `G/projection/` and `G/cypher.rs`; deployment-modes D07 later wraps
  this API in `GraphStore`. Write an unpublished projection build only; verify
  IDs/families/digests/counts/schema/indexes after flush/close/reopen before
  kernel readiness.
- [ ] **Check.** Run both Test commands, a public-API consumer test and
  `capped cargo nextest run -p maestro-kernel store::tests::migrations`. No
  engine I/O in SQLite transactions, lbug types in the public port, or changed
  pinned reads. Catalog edges require catalog authority, not evidence spans.

G27 includes the approved engine follow-up, not just the port. All 14 slices
below inherit its Acceptance requirements and common gates; they are a
breakdown, not additional G-task IDs. Exact file ownership is under `P/`, the
kernel receipt boundary, CLI setup/health/cleanup, the canonicalization safe
filesystem boundary, and the named external repositories. No lane edits Cargo's
downloaded sources. Any slice above four hours is split before dispatch (E01
already uses sub-slices); its range is not an oversized lane authorization.

| Slice | Files/deliverable and failing-first proof | Prerequisites | Estimate from observed lanes |
| --- | --- | --- | --- |
| E01 | lbug fork native filesystem + `maestro-native-safety.yml`: root capability/Unix operations; leaf/ancestor/replacement sentinels, restricted unsupported Windows fails closed. Audit all operations, not opens alone. | D8; API can begin independently | 4–10 h |
| E02 | lbug fork Windows safe-operation implementation; reparse/ancestor/identity/sidecar tests for the full E01 inventory. | E01 API | 4–10 h |
| E03 | lbug fork `build.rs`: external compatible native cache; partial/concurrent/untrusted entries refuse, incompatible keys rebuild. | E03b first for shared build-script ownership | 3–8 h |
| E03b | lbug fork `build.rs`/download helpers: remove automatic downloads; external/env-unset/empty-cache builds succeed with zero fetch attempts on three OSes. | D8 | 2–4 h |
| E04 | rust-workflows policy/workflow/planner: exactly `coverage-features` and `mutation-engine`; unrelated-package and feature-only fixtures prove complete ownership, no extra unviable, default+feature coverage and required harvest. | E03 cache interface; supervisor release/re-pin before E07a | 4–8 h |
| E05 | `P/writer.rs`, `P/content.rs`, shared contract tests: verify/publish/receipt-named open, canonical digest vectors, fixed-width basename, nonterminal verification. | None; landed contract evidence retained | 2–4 h |
| E06 | `K/src/facts/projection.rs`, read-only store inventory, `P/receipts.rs`: old/missing authority never created/migrated; scoped current inventory and exact receipts. | E05 contract agreement; can overlap implementation | 2–4 h |
| E07a | Core pin/lock/vet/ADR/research, feature manifests, engine-owned policy, `lbug-qualification.yml`, `P/engine/open.rs`: rooted smoke and forbidden old constructor lint; default no-lbug and required feature gates from this commit. The frozen `graphdb.*` activation handoff is deferred below (+2 h). | C47a, G01, combined E01/E02/E03/E03b pin, fresh three-OS G25 qualification, released E04 | 5–8 h |
| E07b | `P/engine/{schema,transaction,rows}.rs`: bound full facts/edges, real post-write rollback, catalog access paths and independently frozen digest vectors. | E07a, E05 | 3–6 h |
| E08a | `P/engine/{backend,reader}.rs`: shared fake/native contract, nonterminal verify and physical receipt-file binding. | E07b, E05 | 2–4 h |
| E08b | `P/{lifecycle,access}.rs`, canonicalization safe graph-root facade, CLI setup: guarded no-overwrite publication, one native handle/path, setup-created permanent guards, process death/cancellation. Carry approved backend settings/lock through reader, writer and publication (+1 h). | E08a, E06 | 5–9 h |
| E09 | CLI health and `P/engine/probe.rs`: read-only inventory, try-lock, real two-open probe, only captured OS lock forms classify Locked; absent-engine tests remain killable. | E06, E08b | 2–4 h |
| E10 | `P/cleanup.rs`, canonicalization anchored removal, CLI cleanup/how-to: Retired/Failed-with-receipt preview/confirmation, guard before lookup, no live-reader/unowned deletion or recursive cleanup. | E06, E08b; parallel with E09 | 3–6 h |
| E11 | `S2/research.md`, how-to and required-check receipts: three-OS engine behavior, source-only runtime, complete feature/default/Windows gates and G22 engine packaging. | E04, E07a/E07b/E08a/E08b, E09, E10 | 2–4 h |

The pin moved to `802abe2` in the combined-pin qualification (2026-10-02), then to `e0a1240` in E07a after E01e, then to `02d90e7` for the E07b rollback fix, then to `8bb2f70` for the native cache key and built-in CMake preset, then to `f91b5bb` for source-checked bootstrap cfg cache reuse. E07a wires the first rooted adapter boundary.
Deferred E07a obligation: after S2 and S3 share a branch, native activation reads the frozen `graphdb.*` settings through the S1 resolver.

Ranges are planning estimates, not observed future durations. Basis: the
2026-09-30 ledger records E05 ruling at 01:10, push/review by 02:03, fix at
02:40 and landing at 03:12; E06 dispatched at 02:03, pushed 02:27 and reviewed
02:37, with one fix round before integration. Its 07:37 landing includes a
long integration wait, not measured coding time. The approximately 1–2 h core
lane observations plus one fix round motivate 2–4 h simple-core ranges; native,
shared gate and lifecycle work gets wider ranges. Native cold builds cost about
20 minutes in observed jobs (plan A1 records the actual per-OS spread), so
fork ranges are wider still. With the approved E07a +2 h and E08b +1 h,
the E-slice sum is **43–89 h**; native queues,
three-OS fork CI, review/re-pin and requalification can extend elapsed time.
No completion-date total is inferred from this sum.

Fork/gate path: E01 → E02 → combined fork pin (also E03b → E03) → native
qualification → E07a → E07b → E08a → E08b → E10 → E11; E09 must also finish.
E03 → E04 → gate release/re-pin joins before E07a, as do C47a and G01.
E05 precedes E07b and E06 precedes E08b; E08b gains no new predecessor.
Keep source-default and both safe-operation platforms in the same final
immutable pin. G28 also waits for C47a and consumes the qualified public native
lifecycle through E10, not the fake-only port. The 2026-10-03 13:33 ruling lets
G28 proceed before E11's receipts/how-to; E11 still gates G27's umbrella and
release. Default builds remain featureless; M2 release uses `--features engine`.
E07a and every later native slice update exact mutation ownership in the same
commit. G11 still owns bounded multi-hop behavior; G28 owns resumable
loading, not E11. G25 and G22 retain final qualification obligations.

### G35 [US3, US5] [P] Build deterministic source descriptors beside passages

**Time:** 4 h. **After:** G09, G10, G31.
**Files:** `G/descriptors/types.rs`, `G/descriptors/build.rs`,
`G/descriptors/port.rs`, `G/descriptors/qdrant.rs`, `G/descriptors/tests.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::descriptors`.
**Acceptance:** FR-S2-008, FR-S2-011, FR-S2-019, SC-S2-006; disposable deterministic
claim-first descriptor data under the existing Qdrant/model infrastructure.

- [x] **Red.** Reject unverified/wrong-generation spans, invented defining text,
  missing endpoint context and conflated disjoint source pointers. Freeze text,
  IDs and scope payload vectors; deletion must not lose claim authority. A
  changed builder/model/profile cannot reuse an incompatible projection receipt.
- [x] **Green.** Add the narrow descriptor projection port/Qdrant adapter;
  deterministically compose name/kind/defining sentence and claim predicate plus
  both endpoints' sourced context. Persist separate pointers, collection/pin/
  version/eligibility payloads and builder/embedding/profile digests. Embed with
  the existing gateway; share compatible outputs across arms. No LLM prose,
  identity merge, new library or second vector store. Keep optional descriptor
  readiness distinct so the rule-only pilot needs no embedding model.
- [x] **Check.** Run Test with fake embeddings and synthetic source data; delete
  and recreate the collection from authority/artifacts. Compare canonical text,
  IDs/pointers/payloads and deterministic lookup fixtures, not ANN byte ordering.
  Re-embedding may use the pinned existing embedding profile; extraction or
  generation models and descriptor backups are never required for authority.

### G28 [US5] Load the frozen snapshot with one resumable batch loader

**Time:** umbrella; G28a–G28d total **13 h** (2 + 4 + 4 + 3), replacing the
old 4 + 1 = 5 h, including its cross-slice hour (2026-10-03 16:38 ruling).
**After:** G26, G35, C47a and G27's qualified native lifecycle through E10;
not E11's receipts/how-to (13:33 ruling). Execute G28a → G28b → G28c → G28d.
**Files:** the exact input, binding, loader and CLI targets in the slices below.
**Test:** all slice Test commands below; shared projection/native/CLI suites,
not an input-seam-only pass.
**Acceptance:** FR-S2-008, FR-S2-019, SC-S2-006; the single G25-qualified parameterized
batch loader fills an unpublished projection build; no COPY/CSV or second loader.

The **2026-10-03 13:42 split ruling** makes these ordered implementation
slices, not new independent G tasks. None is accepted alone; all obligations
and the umbrella checkboxes remain open until the integrated G28 evidence
passes. G28 also consumes the deferred E07a frozen `graphdb.*` activation
obligation after S2 and S3 share a branch, through the S1 resolver.

- [ ] **Red.** All four slices' guards have failing-first evidence, including
  both record families, disk-full/repeated/uncertain batches, changed inputs,
  explicit resume refusals and old-reader pin survival.
- [ ] **Green.** One complete snapshot loader uses the four input groups,
  durable binding and immutable checkpoints; no second loader or latest lookup.
- [ ] **Check.** Integrated clean/resumed IDs/digests/counts agree; the real
  rebuild CLI, repair-command parse test, rebuild-time/peak-disk evidence and
  how-to pass without weakening FR-S2-008/019 or SC-S2-006.

#### G28a — resolver-pinned snapshot input seam

**Time:** 2 h planning allocation (2026-10-03 16:38).
**After:** G26, G35, C47a and G27's qualified native lifecycle through E10.
**Files:** `P/import.rs`, `P/tests/import.rs`, `P/mod.rs`, `P/tests/mod.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::projection::tests::import`
and `capped cargo nextest run -p maestro-knowledge graph::projection`.
**Acceptance:** contributes the frozen input seam to G28's FR-S2-008/019 and
SC-S2-006; no independent G28 acceptance or native loading credit.

**Status:** seam done and pushed as `f95edb5` (ledger
`s2-knowledge-graph/g28-part1-report.md`; 2026-10-03 14:08 completion entry).
That commit is not in this amendment's `2516564` baseline; this is not an
integration tick or completion of the umbrella.

- **Red.** Missing/invisible resolution, wrong claim-set membership/content,
  unsupported resolver and denied scope refuse. Assert snapshot-frozen reviews
  and history survive later live changes. Preserve Unicode, quotes/newlines
  and bound-looking values in both families; literals never become endpoints.
- **Green.** Require caller-supplied claim-set and resolution IDs. Verify the
  visible snapshot covers that set and record its resolver version. Reuse
  `graph::resolve::resolve_snapshot` (the 13:36 ruling's `derive`), not another
  resolver or hashes invented from names. Preserve ordered selected membership,
  frozen review states and history, including withdrawn/superseded/retired
  records with status; do not pull in other historical sets' members.
- **Check.** Run both Test commands. No latest lookup, native open, publication
  or identity-encoding change. Durable input binding remains G28b, per the
  13:36 resolution-pin and 13:42 split rulings.

#### G28b — durable projection input pins

**Time:** 4 h planning allocation (2026-10-03 16:38).
**After:** G28a and the landed G26 fixes; inherited G28 inputs still apply.
**Files:** `K/migrations/0030_graph_input_pins.sql`, `K/src/store/migration.rs`,
`K/src/store/tests/migrations.rs`, `K/src/facts/build_types.rs`,
`K/src/facts/projection.rs`, `K/src/facts/tests/projection.rs`, `P/build.rs`,
`P/settings.rs`, `P/schema.rs`, `P/writer.rs`, `P/port.rs`, `P/probe.rs`,
`P/engine/schema.rs`, `P/engine/rows.rs`, `P/engine/reader.rs`,
`P/engine/registry.rs`, `P/engine/probe.rs`, `P/engine/probe_files_tests.rs`,
`P/engine/public_fixture.rs`, `P/tests/lifecycle.rs`, `P/tests/contract.rs`,
`P/tests/projection_writer.rs`, `P/tests/cleanup_support.rs`, `P/cleanup.rs`,
`N/tests/it/projection_lifecycle/fixture.rs`,
`N/tests/it/projection_lifecycle/native_processes.rs`,
`C/src/cli/graph/tests/cleanup_support.rs`, `C/tests/it/graph_cleanup_support.rs`,
`C/src/cli/health/graph_native.rs`, `C/src/settings/session.rs`,
`crates/maestro-catalog/src/settings/preferences.rs`.
**Test:** `capped cargo nextest run -p maestro-kernel facts::tests::projection`,
`capped cargo nextest run -p maestro-kernel store::tests::migrations`,
`capped cargo nextest run -p maestro-knowledge graph::projection`,
`capped cargo nextest run -p maestro graph_operations`,
`capped cargo nextest run -p maestro settings` and
`capped cargo nextest run -p maestro-catalog settings`; native projection tests
with `--features engine`, with three-OS qualification evidence from CI.
**Acceptance:** contributes durable build/readiness/native binding to G28's
FR-S2-008/019 and SC-S2-006; closes G26's deferred durable-binding obligation,
not G28's loader/rebuild acceptance.

- **Red.** Test changed settings/lock, resolution or snapshot mismatch, old
  native stamp, missing/unknown/malformed pins and matching-pin open. A newer
  resolution must not invalidate a published graph. Upgrade a legacy database
  without losing `/1` rows; refuse invalid `/2` pins and a resolution that does
  not cover the claim set. Prove the persisted lock equals the admitted one,
  killing the five G26 `frozen_lock` mutants, not only testing live equality.
- **Green.** Per the 12:08, 13:36 and 15:30–15:31 rulings, add required
  `resolution_id`, resolver version, `graph-settings/1` typed settings identity
  and complete frozen-lock digest to `ProjectionBuild` and receipts; stamp the
  same pins in native `maestro-typed-edges/2`. The tagged canonical settings
  digest covers the three admitted D14 integers, with a golden test; the lock
  digest is separate. Rebuild `graph_projection_receipts` in append-only 0030:
  `/1` requires NULL pins, `/2` all four well-formed/non-NULL; copy legacy rows
  as `/1`, recreate indexes/triggers, add the resolution foreign key and
  BEFORE INSERT coverage trigger. No `graph_projection_builds` table.
- **Check.** Strict decoding refuses legacy unpinned `/1`; open/probe compares
  native stamps to receipt pins and receipt settings/lock to current admission.
  Preserve `reader`/`reader_cancellable`: the receipt supplies resolution,
  never a latest lookup or new resolution argument. Verify pins separately
  from `BuildVerification` counts/content. Health changes "not yet bound" to
  per-receipt bound or mismatch plus the shared `maestro graph rebuild` repair.
  The 15:58 ruling uses `ProjectionError::InputMismatch` and
  `ProbeError::InputMismatch` with shared `Settings`/`Lock`/`Resolution`/`Format`
  kinds only for pin inequality or legacy formats. Display contains kind and
  repair only, no values/digests/paths/backend text; corruption keeps its own
  error. Assert variant and kind. Audit every build/receipt site and serialized
  readiness fixture: the G28a report's five build and twelve receipt **sites**
  include definitions/signatures, not five/twelve literal constructors.

#### G28c — one loader with immutable checkpoints and explicit resume

**Time:** 4 h planning allocation (2026-10-03 16:38).
**After:** G28b; consume G28a's input seam and the existing native lifecycle.
**Files:** `P/import.rs`, `P/tests/import.rs`, `P/lifecycle.rs`,
`P/engine/producer.rs`, `P/engine/backend.rs`, `P/adapter/mod.rs`,
`P/checkpoint.rs`, `P/tests/checkpoint.rs`, `P/tests/lifecycle.rs`,
`N/tests/it/projection_lifecycle/fixture.rs`,
`N/tests/it/projection_lifecycle/native_processes.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::projection`,
`capped cargo nextest run -p maestro-knowledge --features engine graph::projection`
and `capped cargo nextest run -p maestro-knowledge --features engine --test it projection_lifecycle`.
**Acceptance:** contributes the single qualified loader and clean/resumed
IDs/digests/counts equality to G28's FR-S2-008/019 and SC-S2-006; G28d still gates
umbrella acceptance.

- **Red.** Load subject facts and entity edges, checking separate counts/digests
  and no literal node/edge. Inject disk-full, repeated batch and kill after
  native commit before checkpoint/receipt. Refuse changed snapshot/resolution/
  settings/lock, stale/foreign lease, corrupt manifest, checkpoint gap/order,
  duplicate/unknown/unparsable file and durable-content mismatch. Published or
  unrelated builds never resume; old reader pins survive.
- **Green.** The 13:34 and 13:37 rulings permit one explicit `resume()` on the
  existing factory for loader-owned unpublished builds. Use job-derived
  deterministic staging, create-new on first start, not PID/counter paths.
  One immutable versioned manifest pins scope, claim set, resolution/version,
  settings identity and lock. After durable verification, write zero-padded
  ordinal immutable checkpoints containing ordinal and verified IDs/digests;
  bound their count without inventing a new default here. Reuse held-root
  `Directory` create/write/sync/`publish_verified`, not replacement helpers.
  Feed both families through existing bounded `write_batch`/`verify`/`publish`.
- **Check.** Refresh the current job lease, fence holder/number after takeover,
  compare every manifest pin and checkpoint to durable content, and construct
  a fresh backend. Recheck the uncertain last batch before idempotent completion
  by key or refusal naming rebuild repair. Never silently reuse/delete staging
  or relax unrelated-orphan recovery refusals. Clean up through the existing
  path only after publish/explicit discard. Close/reopen and verify both
  families before readiness; clean and resumed IDs/digests/counts agree. Keep
  the native qualification gates; no COPY/CSV, second loader or live-file copy.

#### G28d — rebuild CLI, integration, measurement and how-to

**Time:** 3 h planning allocation (2026-10-03 16:38).
**After:** G28c; use G35's optional descriptor seam, not another builder.
**Files:** `C/src/cli/graph/rebuild.rs`, `C/src/cli/graph/mod.rs`,
`C/src/cli/args.rs`, `C/src/cli/run.rs`, `C/tests/it/graph_rebuild.rs`,
`docs/how-to/knowledge-graph.md`, `S2/research.md`.
**Test:** `capped cargo nextest run -p maestro graph_rebuild` and
`capped cargo nextest run -p maestro-knowledge graph::projection`; engine-enabled
CLI/integration runs use `--features engine` and the existing CI qualification.
**Acceptance:** completes G28's CLI/integration/measurement obligations toward
FR-S2-008/019 and SC-S2-006 only with G28a–G28c's integrated evidence; no slice
is accepted alone, and G30/G22 retain their own equality/restore obligations.

- **Red.** Test the literal command from G28b's shared rebuild-repair constant
  through the real CLI parser: **`maestro graph rebuild` must parse** (15:31
  ruling; G28a report's last section). Test explicit/default resolution display,
  refusal before changed-input replay, interrupted rebuild and old-reader pins.
- **Green.** Wire that rebuild command to G28c's sole loader and G28a/G28b pins.
  Show the chosen resolution, including any CLI default, without adding a
  latest lookup to readers or the loader. Orchestrate G35's optional descriptor
  rebuild/readiness from the same frozen snapshot/profile; the rule-only pilot
  leaves it disabled. Preserve owned-root and publication boundaries.
- **Check.** Run end-to-end tests and record this loader's rebuild time and peak
  disk in `S2/research.md`, then document the runnable repair and explicit
  resume flow. G11 retains the later explicit scale benchmark; invent no new
  ceiling. Graph-file rebuild, including G36's later transitions, needs no
  model, Qdrant claim authority or live-file copy. Only descriptor re-embedding
  may use the pinned existing embedding profile; backup/restore requires no
  descriptor projection storage. G36 extends this one loader, not a second
  engine. G28 must land whole before S2 reaches main (15:31 ruling).

G29 is removed: resuming G28 is not a separate incremental implementation.
A second loader is outside S2; proposing it later requires a supervisor-set
100,000-edge build-time ceiling registered before the measurement, and a miss.

### G04 [US1] Return scoped neighbors from the pilot

**Time:** 4 h. **After:** G03, G25, G28.
**Files:** `P/read.rs`, `P/tests/neighbors.rs`, `G/query.rs`,
`G/tests/neighbors.rs`, `K/src/facts/read.rs`, `K/src/facts/quote.rs`,
`C/src/cli/graph/read.rs`, `C/src/cli/args.rs`, `C/src/cli/run.rs`,
`C/tests/it/graph_neighbors.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge neighbors` and
`capped cargo nextest run -p maestro graph_neighbors`.
**Acceptance:** FR-S2-007, FR-S2-010, FR-S2-014, SC-S2-001; versioned CLI
neighbors read through LadybugDB's projection port and carry claims, revision,
original span/quote and explicit ambiguity/coverage.

- [ ] **Red.** Assert hidden/unknown ID parity, current grants, version
  selection, ambiguous names, deterministic ties, bounded work and cancellation
  in CLI JSON, projection reads and kernel evidence checks. No result means
  no documented graph link,
  not no relation in the corpus.
- [ ] **Green.** Project frozen verified pilot claims through G27/G28. Return
  the subject's `entity_facts` plus typed neighbors when entity edges exist,
  only from LadybugDB behind G27's port. The literal-only pilot returns fact
  records with no invented object entity; path length counts entity edges only. Keep claim authority
  and final evidence checks in the kernel, never a SQLite graph query. Missing
  or locked LadybugDB is unavailable, not a fallback; G11 extends this engine
  with paths rather than replacing a temporary traversal implementation.
- [ ] **Check.** Run both Test commands and query the G01 fixture after import
  and build with plan A0's temporary declaration/manifest and version `1.0`.
  Confirm every emitted byte span/quote and no hidden title/count. Assert
  absent LadybugDB never dispatches a SQLite neighbor/path query.

### G32 [US1, US4] [P] Run independent local review with private capture

**Time:** 4 h. **After:** G04, G06, G34.
**Files:** `N/src/eval/graph/review.rs`, `N/src/eval/graph/tests/review.rs`,
`C/src/cli/eval/graph_review.rs`, `C/src/cli/eval/manifest.rs`,
`C/src/cli/args.rs`, `C/src/cli/run.rs`, `C/tests/it/graph_review.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge eval::graph` and
`capped cargo nextest run -p maestro graph_review`.
**Acceptance:** FR-S2-018, FR-S2-021, SC-S2-001, SC-S2-003; one bounded local
review runner writes private dispositions without exposing source text.

- [ ] **Red.** Refuse nonlocal/unpinned cards, reviewer family matching the
  extractor/drafter, insufficient `Room::Free`, missing approval/scratch
  bindings, a private destination outside its approved root, malformed review
  output and mismatched input digests. Capture
  standard output and standard error on success, malformed input, timeout and
  nested failures: only aggregates, fixed codes, item IDs and digests escape.
- [ ] **Green.** Add `maestro eval graph review --manifest` over the existing
  gateway/router and graph-read ports, with pinned local card/family, prompt,
  input and receipt digests, per-item budgets and resumable progress. In pilot
  mode read entity IDs internally from the frozen receipt, invoke G04's reads
  and write their full JSON to `pilot-results.json` under the approved PRIVATE
  root before review. For claims/chains, consume manifest-bound private inputs.
  Write every review/flag/failure to private JSONL; stdout/stderr never contain
  names, labels, quotes or raw model/router errors. No private file is read
  back into lane context. A model disposition is not an owner's ruling.
- [ ] **Check.** Run both Test commands with synthetic inputs and the fake
  gateway. Every input ID has a disposition or retained failure; retries do
  not lose/duplicate items. Rule-made claims use the separately pinned local
  reviewer; where extraction/drafting used a model, its family must differ.
  Rerun G06's default-kernel and sanitized-error checks. No new runner library.

### G33 [US1, US4] [P] Render the owner's local review page

**Time:** 4 h. **After:** G32.
**Files:** `C/src/cli/eval/graph_review_page.rs`,
`C/src/cli/eval/tests/graph_review_page.rs`, `C/src/cli/args.rs`,
`C/src/cli/run.rs`, `C/tests/it/graph_review_page.rs`.
**Test:** `capped cargo nextest run -p maestro graph_review_page`.
**Acceptance:** FR-S2-018, FR-S2-021, SC-S2-001, SC-S2-003; T027-style local
owner rulings on flagged claims/chains, without a service or hosted review.

- [ ] **Red.** Escape hostile source/quote/model text in HTML and decision
  exports. Refuse stale input/card digests, unknown item IDs, duplicate rulings
  and decisions outside the approved scope. Unresolved flags cannot freeze.
  Verify no private text reaches stdout/stderr or any network resource.
- [ ] **Green.** Add `maestro eval graph review-page --manifest`, consuming
  G32's private receipts and writing a self-contained static page under PRIVATE.
  The owner reads flags locally and exports a local decision JSON file; the
  same command with `--decisions` validates and records those rulings by item
  ID/input digest. Preserve original model dispositions and owner history.
  No HTTP server, external assets, automatic approval or new UI framework.
- [ ] **Check.** Run Test with synthetic flagged claims and chains. A complete
  owner disposition survives export/import; changed inputs invalidate it.
  Only counts, IDs and digests leave the local review workflow. G05/G08/G20/G23
  use this page rather than asking a hosted lane to inspect private text.

### G05 [US1] Independently check the private pilot

**Time:** 3 h. **After:** G03, G04, G32, G33; approved pilot scope/receipt and
owner-confirmed local reviewer protocol.
**Files:** `PRIVATE/graph/select-pilot.py`, `PRIVATE/graph/rules/defaults.json`,
`PRIVATE/graph/receipts/pilot-inputs.json`,
`PRIVATE/graph/receipts/pilot-review.jsonl`,
`PRIVATE/graph/receipts/pilot-results.json`,
`PRIVATE/graph/receipts/pilot-review-manifest.json`,
`PRIVATE/graph/receipts/pilot-review.html`,
`PRIVATE/graph/receipts/pilot-decisions.json`, `PRIVATE/graph/receipts/scratch.json`.
**Test:** `maestro eval graph review --manifest "$PRIVATE/graph/receipts/pilot-review-manifest.json"`
in pilot-capture mode, with plan Validation's explicit scratch bindings;
`maestro eval graph review-page --manifest "$PRIVATE/graph/receipts/pilot-review-manifest.json"`
writes the owner-only local page. Only aggregates, IDs and digests are printed.
**Acceptance:** SC-S2-001, FR-S2-021; every accepted claim and its quote has an
independent review disposition; no private material is copied to core or CI.
**Executor / inference:** lane-run Maestro commands expose aggregate metrics,
IDs and digests only; no private text enters hosted context. A different local
model family from the router catalog reviews every rule-made claim; the owner
rules on flags on a local review page. No extraction model runs in the pilot.

- [ ] **Red.** Refuse missing/overlapping scratch bindings, wrong backup
  digest, changed/expired approval, missing scope/target/expiry/evidence fields,
  unapproved version and a deliberately false relation.
- [ ] **Green.** Apply the 12:53 approved pilot scope: one 9.0.22 parameter
  table selected by the local deterministic script. Freeze script/input/window
  digests in the scope/target/expiry/evidence receipt before reading its text
  for extraction/review. Restore a named backup with isolated kernel/Qdrant
  bindings, then import/build/project there through G09/G28. Redirect both
  output streams of these prerequisite commands to private receipt files;
  never read them in lane context or open the live kernel. G32 reads
  entity IDs internally from the frozen receipt and writes full neighbors
  JSON directly to private `pilot-results.json`, never the lane's terminal.
  Run its independent local review and G33's owner-page/decision import.
- [ ] **Check.** Run the Test query matrix, including denied/unknown and
  ambiguous cases. Reconcile counts and input/candidate/output digests using
  G32's sanitized summary; the lane never opens raw result or review files.
  Require every claim's disposition and owner flag ruling. Retain failures
  privately rather than changing source truth to fit the rule.

**Checkpoint:** the rule-only LadybugDB pilot and its prerequisites total
the recomputed totals in the dependency accounting below, including G35 and
G06/G31/G32/G33/G34; G27 engine work/waits are accounted separately.
G05 gates G07/G08/
G20/G23, not public G10 implementation. Inspect failures before private
acceptance; pilot success is not M2.

## Phase 3: User Stories 2 and 3 — admissible paths and complete evidence

### G11 [US2] [P] Extend LadybugDB neighbors with bounded Cypher paths

**Time:** 4 h. **After:** G04, G25, G28, G31.
**Files:** `G/query.rs`, `G/cypher.rs`, `G/tests/cypher.rs`,
`G/tests/paths.rs`, `P/port.rs`, `P/read.rs`, `K/src/facts/recheck.rs`,
`K/src/facts/tests/recheck.rs`, `C/src/cli/graph/read.rs`,
`N/tests/it/graph_scale.rs`, `S2/research.md`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::tests` and
`capped cargo nextest run -p maestro-kernel recheck`.
**Acceptance:** FR-S2-009, FR-S2-010, SC-S2-005; LadybugDB neighbors and
shortest admissible paths, every result rechecked in the kernel.

- [ ] **Red.** Test hidden/ineligible middle hops, forbidden shortest versus
  allowed longer path, revocation after admission, cycles, fan-out, unknown
  conditions, version boundaries, depths four/five, ties, injection, native
  cancellation and locked/missing/stale graphs. Catalog edge families and
  literal defaults cannot become documentary multi-hop proof links.
- [ ] **Green.** Bind fixed Cypher templates; filter scope/eligibility/
  generation/version/conditions at every hop before ranking and limits. Bound
  native expansion and time as well as output. Recheck every entity, claim and
  support against current kernel grants/eligibility. Extend G04's LadybugDB
  reads behind the same projection port; SQLite supplies only claim authority/
  export and evidence checks, never neighbor/path queries.
- [ ] **Check.** Run Test on small fixtures. Run the ignored 10,000/100,000-edge
  profile explicitly, outside per-mutant runs:
  `capped cargo nextest run -p maestro-knowledge --test it --run-ignored only graph_scale`.
  Generate these fixtures at test time, never commit them (HYG-003). Mark the
  test `#[ignore]`; record build time, expansion caps and cleanup.
  Inaccessible and unknown responses match; no graph failure enables SQL or
  Neo4j fallback, and limited traversal never claims corpus-wide absence.

### G36 [US2, US3, US5] Add source-derived passage transitions

**Time:** 4 h. **After:** G11, G28, G10.
**Files:** `G/passages/types.rs`, `G/passages/derive.rs`, `G/passages/port.rs`,
`G/passages/tests.rs`, `P/port.rs`, `P/schema.rs`, `P/import.rs`, `P/read.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::passages` and
`capped cargo nextest run -p maestro-knowledge graph::projection`.
**Acceptance:** FR-S2-007, FR-S2-008, FR-S2-009, FR-S2-010, FR-S2-023, FR-S2-025,
SC-S2-006; verified source transitions form retrieval hints, never proof claims.

- [ ] **Red.** Refuse unresolved cross-references, unverified mentions and
  hidden/wrong-version intermediate passages before caps. Test cycles, fan-out,
  cancellation, old pins, unauthorized/unknown parity and derivation rebuild
  equality. A transition or catalog edge must never enter a documentary proof.
- [ ] **Green.** Derive stable passage-ID structural/cross-reference links and
  inverted verified-entity membership from authority. Extend G28's one loader
  and Ladybug port with a separate retrieval family through a narrow passage
  adapter. Generate shared neighbors on demand rather than a quadratic clique.
  Keep source provenance; no relation-semantic review is required for hints.
- [ ] **Check.** Run both Test commands. Enforce configurable seed/hop/per-entity
  fan-out/visited/time limits, pre-cap filters and current-authority rechecks;
  disclose bounded coverage. Cross-version expansion needs an explicit request.
  Rebuild deterministic transition content/IDs/results, preserving other families.

### G12 [US3] [P] Version graph evidence without weakening S1

**Time:** 3 h. **After:** G04, G06.
**Files:** `K/src/evidence/graph.rs`, `K/src/evidence/tests/graph.rs`,
`K/src/evidence/tests/search_wire.rs`, `N/src/search/request.rs`,
`N/src/search/tests/handoff.rs`, `N/src/eval/graph/score.rs`,
`N/src/eval/graph/tests.rs`.
**Test:** `capped cargo nextest run -p maestro-kernel evidence` and
`capped cargo nextest run -p maestro-knowledge handoff`.
**Acceptance:** FR-S2-012; strict graph `/2` round trips and proof metadata
carried through `EvidenceInput`, with non-graph `/1` unchanged.

- [ ] **Red.** Reject graph fields in `/1`, unknown/duplicate `/2` keys,
  dangling claims/passages, duplicate supports and mixed-generation proofs.
  Ensure handoff retains every link even when intermediate candidates rank low.
- [ ] **Green.** Add versioned graph types with claims, paths, supports,
  attachment identity, coverage and known gaps. Reuse S1 provenance rather than
  copying it into another store; no permissive decoder fallback.
- [ ] **Check.** Run both Test commands and the old `/1` wire fixtures.
  Serialization/deserialization checks the entire proof contract, and an
  incomplete proof cannot be labeled complete by a caller-written flag.

### G13 [US3] Assemble and budget whole proof groups

**Time:** 4 h. **After:** G11, G12.
**Files:** `N/src/search/evidence/graph.rs`,
`N/src/search/evidence/assemble/engine.rs`,
`N/src/search/evidence/assemble/output.rs`,
`N/src/search/evidence/budget.rs`, `N/src/search/evidence/versions.rs`,
`N/src/search/evidence/tests/graph.rs`,
`N/src/search/evidence/tests/graph_budget.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge search::evidence`.
**Acceptance:** FR-S2-013; all required links, conditions, versions and citation
metadata survive assembly or become explicit sanitized gaps.

- [ ] **Red.** Rechunk source spans, revoke one intermediate support, overlap
  two supports and cross exact-fit/one-over budgets. Test different versions,
  contradictory claims and an exact counter failure; never silently use bytes
  in place of a failed exact count.
- [ ] **Green.** Map anchors into the pinned chunk set and reserve whole proof
  units through selection/merging. Charge passages plus graph metadata; keep
  source spans stable and duplicate copies from becoming extra votes.
- [ ] **Check.** Run Test; the retained proof is whole or absent with a gap.
  Emit candidate/pre-fusion/post-packing attrition and citation/support coverage
  with denominators; route reachability is not a complete delivered proof.
  Every passage/claim reference resolves and no high-scoring endpoint replaces
  an omitted intermediate link. Non-graph assembly retains its baseline.

### G14 [US3] [P] Add question-seeded R4 to the existing fusion

**Time:** 4 h. **After:** G13, G35.
**Files:** `N/src/query/understand.rs`, `G/seeds.rs`,
`N/src/search/routes/graph.rs`, `N/src/search/fusion.rs`,
`N/src/search/request.rs`, `N/src/search/admission.rs`,
`N/src/search/route_execution.rs`, `N/src/search/orchestrate.rs`,
`N/src/search/tests/graph.rs`, `N/src/search/tests/fusion.rs`,
`C/src/cli/eval/manifest.rs`, `C/src/cli/eval/tests/manifest.rs`,
`C/src/cli/eval/graph.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge search::tests` and
`capped cargo nextest run -p maestro cli::eval::tests::manifest`.
**Acceptance:** FR-S2-009, FR-S2-010, FR-S2-011, SC-S2-002, SC-S2-005; FR/EN question-only seeds,
complete support handoff, deterministic fusion and graph-disabled parity.

- [ ] **Red.** Assert one-based RRF/ties, no duplicate echo vote, no seed from
  dense/BM25 hits, exact disabled-search parity and unavailable/deadline
  behavior without suppressing passage routes. Graph `none` makes zero calls,
  including opens/probes, and is not reported unavailable. Pre-fusion recall is
  diagnostic only; complete-proof gain uses the delivered evidence bundle.
  Add EN→EN/FR→FR/FR→EN/EN→FR NIL, homonym, paraphrase and competing-endpoint
  cases. Hidden/wrong-version high-score descriptors must not exhaust top-k:
  assert Qdrant collection/pin/version/eligibility filters before lookup caps,
  scope-representation failure closed and revocation recheck before traversal.
- [ ] **Green.** Add Graph once to the route/configuration/observation seams.
  Use exact names/identifiers/reviewed aliases first; whenever no exact seed
  exists, use G35's direct question-to-claim/triple descriptors before entity
  names under the selected profile. No undefined question classifier or online
  translation/NER. Similarity neither writes identity nor adds a truth vote.
  Start development with at most five seeds; freeze direction-specific thresholds
  and caps in G38, not per-product rules. Preserve whole proofs, the existing
  reranker and one shared 20 s deadline, including embedding/queue work.
- [ ] **Check.** Run both Test commands. Missing/stale/locked/rebuilding graph
  yields a named unavailable reason and no hidden fallback. The existing
  retrieval fixture baseline does not regress when graph is disabled.

### G15 [US2, US3] [P] Expose graph reads with whole-proof transport limits

**Time:** 4 h. **After:** G11, G13.
**Files:** `C/src/knowledge/requests.rs`,
`C/src/knowledge/operations/graph.rs`, `C/src/knowledge/output/policy.rs`,
`C/src/knowledge/output/tests.rs`, `C/src/mcp/graph_tools.rs`,
`C/src/mcp/server/handler.rs`, `C/src/cli/graph/read.rs`,
`C/src/cli/args.rs`, `C/tests/it/graph_mcp.rs`, `docs/how-to/knowledge-graph.md`.
**Test:** `capped cargo nextest run -p maestro graph_mcp` and
`capped cargo nextest run -p maestro knowledge::output`.
**Acceptance:** FR-S2-010, FR-S2-013, FR-S2-014, SC-S2-008;
neighbors/path/entity-resolve/evidence-trace
share one scoped operation across CLI/MCP and return no SQL/Cypher interface.

- [ ] **Red.** Test ambiguous names, explicit version/generation, current
  revocation, timeout and hidden/unknown parity. Make an intermediate support
  the largest field in a proof crossing the complete framed 64 KiB limit.
- [ ] **Green.** Add four bounded tools and CLI equivalents over shared
  operations. Replace passage-wise truncation for graph `/2` with complete
  proof-group removal and disclosure; recheck authorization before output.
- [ ] **Check.** Run both Test commands with actual stdio framing and `/1`
  regression cases. No dangling support survives a limit. Record synthetic
  Pi/Claude Code smoke evidence; Copilot needs owner approval. Private
  responses never enter public receipts.

### G37 [US3] Fuse dependent passage candidates without echo votes

**Time:** 4 h. **After:** G14, G15, G36.
**Files:** `N/src/search/routes/passages.rs`, `N/src/search/fusion.rs`,
`N/src/search/route_execution.rs`, `N/src/search/orchestrate.rs`,
`N/src/search/request.rs`, `N/src/search/tests/passages.rs`,
`C/src/cli/eval/manifest.rs`, `C/src/cli/eval/tests/manifest.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge search` and
`capped cargo nextest run -p maestro cli::eval::tests::manifest`.
**Acceptance:** FR-S2-013, FR-S2-025, SC-S2-005; newly discovered original
passages enter existing fusion/reranking with dependent origin accounting.

- [ ] **Red.** Assert admitted dense/lexical seeds, no extra vote for unchanged
  echoes, stable source deduplication and parent attribution. Hidden candidates
  cannot consume caps. Test high-degree hubs, exhausted deadline, disabled zero
  calls and unavailable transitions independently of R4. R4 stays question-only.
- [ ] **Green.** Call G36's bounded adapter using admitted passage hits; carry
  only newly discovered source passages through the existing reranker and budget
  path. Retain origins/coverage, never present a transition as a claim/citation.
  Reuse the admission deadline and support guards; no extra model or context.
- [ ] **Check.** Run both Test commands, graph-disabled baseline and final-wire
  support tests. All route switches/bounds are profile-bound; disabling this
  route never alters its callers or hides an unavailable selected graph.

## Phase 4: User Stories 3 and 4 — small-model answers and extraction

### G16 [US3] Answer only from complete cited graph proofs

**Time:** 3 h. **After:** G14, G15, G37; integrated S1 ask.
**Files:** `N/src/answer/prompt.rs`, `N/src/answer/generate.rs`,
`N/src/answer/validate.rs`, `N/src/answer/types.rs`,
`N/src/answer/tests/graph.rs`, `C/src/knowledge/operations/ask/run.rs`,
`C/tests/it/knowledge_ask.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge answer` and
`capped cargo nextest run -p maestro knowledge_ask`.
**Acceptance:** FR-S2-013, FR-S2-015, SC-S2-003, SC-S2-004, SC-S2-005; at-most-4B local answerer,
card/settings frozen for acceptance runs, uncalibrated scores and at most one retry.

- [ ] **Red.** Reject invented commands/links, incomplete citations, inferred
  transitivity, unavailable required graph proof and unresolved contradictory
  versions. A second bad answer must refuse, not request another retry.
  Simulate queue/loading/retrieval/first-attempt/validation elapsed time and
  prove the retry receives only the remaining 20 s request budget; expiration
  cancels work and earns no correct-refusal credit. Passage hints cannot prove
  relationships; final unsupported conclusions and lost supports are scored.
- [ ] **Green.** Map proposed conclusions to complete proof groups before
  generation; extend existing buffered citation/command guards through final
  kernel rechecks. Keep the question's language and exact source commands.
- [ ] **Check.** Run both Test commands; no incomplete proof becomes a verified
  answer before or after wire limits. Report local answer latency and refusal
  results in later frozen evaluation; do not claim general calibration here.

### G17 [US4] [P] Add the qualified extractor role to the model registry

**Time:** 4 h. **After:** G01; decided D2.
**Files:** `K/src/gateway/card_types.rs`,
`K/src/gateway/card_v2/validation.rs`, `K/src/model/records.rs`,
`K/src/model/write.rs`, `K/src/model/read.rs`,
`K/src/model/tests/extractor.rs`, `K/src/gateway/tests/extractor_card.rs`,
`K/migrations/NNNN_extractor_role.sql`, `K/src/store/migration.rs`.
**Test:** `capped cargo nextest run -p maestro-kernel extractor`.
**Acceptance:** FR-S2-016; distinct Extractor registration, evaluation and
selection while existing role histories, pinned cards and guards survive.

- [x] **Red.** Upgrade a populated S1 database; test cards/evaluations/
  selections for the new role, old roles, wrong-role card selection and
  synthetic-as-real refusal. Reopen and verify unchanged card digests/pins.
- [x] **Green.** Forward-migrate all three role CHECK constraints, preserving
  immutable records, foreign keys and real eligible evaluation guards. Extend role/card
  validation without treating Answerer qualification as Extractor qualification.
- [x] **Check.** Run Test and `capped cargo nextest run -p maestro-kernel model`.
  A failing upgrade rolls back, old cards still read unchanged, and only a
  real qualified extractor evaluation permits production selection.

### G18 [US4] Make one bounded constrained extraction call

**Time:** 4 h. **After:** G17.
**Files:** `K/src/gateway/extract.rs`, `K/src/gateway/port.rs`,
`K/src/gateway/router.rs`, `K/src/gateway/fake.rs`,
`K/src/gateway/tests/extract.rs`, `K/src/gateway/tests/chat.rs`.
**Test:** `capped cargo nextest run -p maestro-kernel gateway`.
**Acceptance:** FR-S2-016, FR-S2-024; closed-schema extraction, pinned card/sampling,
`Room::Free` and at most 1,024 output tokens without weakening Answerer chat.

- [x] **Red.** Inspect stub-router wire JSON and refusal paths: wrong role,
  card mismatch, invalid/partial JSON, duplicate/unknown keys, unsupported
  sampling/template controls, output overflow and insufficient room.
- [x] **Green.** Add the constrained extraction request through the existing
  port/router and deterministic fake. Derive typed candidates from architecture
  02 §8.2's closed lists and FR-S2-024, excluding `ALIAS_OF` claims; no tools,
  arbitrary JSON instructions or caller-chosen authority fields. Reject an
  instance `Job` kind and event-satisfaction-as-dependency shortcuts.
- [x] **Check.** Run Test; no room means no unload, invalid output admits no
  partial claims, and all existing embed/rerank/tokenize/chat tests still pass.

### G19 [US4] [P] Extract offline through the verified build pipeline

**Time:** 4 h. **After:** G09, G10, G18, G31.
**Files:** `G/extract/windows.rs`, `G/extract/run.rs`,
`G/extract/tests.rs`, `G/build/run.rs`, `C/src/cli/graph/build.rs`,
`C/tests/it/graph_extract.rs`, `PRIVATE/graph/prompts/extract.md`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::extract` and
`capped cargo nextest run -p maestro graph_extract`.
**Acceptance:** FR-S2-017, FR-S2-003, FR-S2-005, FR-S2-024; bounded offline candidates use
G03's block/quote checks and G02's authoritative write, with retained rejections.

- [ ] **Red.** Test negation, Unicode, repeated ambiguous quotes, misleading
  source instructions, window/output/token overflow and retry after a crash.
  Rejected JSON, unknown kinds/predicates or `ALIAS_OF` claims outside
  architecture 02 §8.2 / FR-S2-024 cannot be accepted.
  A held-out label/failure must never influence window-policy tuning.
- [ ] **Green.** Tune the bounded window policy on pilot/synthetic development
  failures only, then apply it across the separately approved acceptance scope;
  tuning receives no held-out labels/results. Freeze prompt/card/window/profile
  digests in G09 jobs. Call G18 outside transactions and retain
  candidates/rejections before advancing durable progress.
- [ ] **Check.** Run both Test commands; clean and resumed runs respect the
  same cumulative work/token budget. Public tests use synthetic prompts only;
  no answer, summary or model-invented quote becomes source evidence. Keep
  documentary scope: no instance `Job` or event satisfaction coerced into a
  dependency; the G18/G31 refusal checks also hold through this pipeline.
  Before broad extraction, verify G38 step 0's registered asset/card/licence
  digests and receipt before the first extractor call; missing/mismatched
  receipts refuse. Use G38's Executor / inference guards and plan Validation's
  scratch environment. Time a fixed token-length-stratified development window sample with input/output tokens, wall/GPU seconds, peak VRAM/RSS,
  rejections and retries. A 30-minute reservation is a checkpoint, not throughput
  evidence. Apply plan A6's actual-window sensitivity and C×f×t/60 owner-hours
  accounting; document unknown rates instead of calling 13,596 units windows.

### G38 [US4] Select linking and passage fusion on development only

**Time:** 4 h active lane work; GPU execution/review waits are measured separately.
**After:** G05, G06, G16, G19, G35, G36, G37 for selection; step 0 below is
an earlier inference-free handoff, not a wait for completed G19.
**Files:** `N/src/eval/graph/development.rs`, `N/src/eval/graph/tests/development.rs`,
`C/src/cli/eval/manifest.rs`, `C/src/cli/eval/tests/manifest.rs`,
`tests/fixtures/synthetic/graph/development.jsonl`, `S2/research.md`,
`PRIVATE/graph/receipts/development-manifest.json`,
`PRIVATE/graph/receipts/development-selection.json`,
`PRIVATE/graph/receipts/extractor-card.json`.
**Test:** `capped cargo nextest run -p maestro-knowledge eval::graph` and
`capped cargo nextest run -p maestro cli::eval::tests::manifest`; then the
existing `maestro eval ladder --manifest` on the approved development manifest,
with plan Validation's scratch environment.
**Acceptance:** FR-S2-011, FR-S2-016, FR-S2-018, FR-S2-021, FR-S2-025, SC-S2-002, SC-S2-004,
SC-S2-005; a frozen profile chosen without held-out questions or failures.
**Executor / inference:** lane-run Maestro commands expose aggregate metrics,
IDs and digests only; extraction and answering use local models through the
router, never hosted context. Independent review uses the pinned different local
model family and G33's owner flag rulings. Use plan Validation's scratch
restore/bindings and refuse missing, expired or overlapping private receipts.

- [ ] **Red.** Refuse overlapping development/acceptance families, unapproved
  private inputs, changed models/budgets between arms or selecting C from
  held-out outcomes. Refuse missing or mismatched asset/card/licence digests
  before the first inference call. Oracle source chains must fit closed vocabulary and final
  budgets before retrieval is judged; gold-seeded results are upper bounds.
- [ ] **Step 0 — pre-use receipt.** Dispatch as soon as the approved candidate's
  exact assets/licence evidence and model-registry registration inputs exist,
  before G19's reopened calibration. Register asset, card and licence digests
  in the model registry and write `extractor-card.json` with those digests and
  GPU cost evidence (D2, ENF-012), without inference. G38 is the sole receipt
  writer; G19 and G20 verify it before calls. Repeat this pre-use step for any
  I1-approved 8B candidate. Selection still waits for completed G19.
- [ ] **Green.** Bind every candidate call to the step-0 receipt. Start A, A+R4,
  A+transitions and A+R4+transitions together on
  synthetic/public development, not after R4 fails. Within R4 compare exact
  versus claim-first linking and rules versus rules+4B; select the extractor
  using the approved real pilot development receipt with synthetic checks,
  before freeze. I1 alone can trigger 8B. No extra private multi-hop data
  without a disjoint development receipt. Private commands retain raw outputs
  under PRIVATE and expose only aggregates, IDs, digests and fixed error codes.
  Share identical embedding/reranker/reader/weights/context/wire/deadline profiles.
  Measure bridge recall, delivered proofs, lookup support/refusals, per-direction
  seed/NIL errors, trigger cost, latency and indexing/review cost by route.
- [ ] **Check.** Run Test and retain every development attempt. Freeze chosen
  C route combination, optional semantic linking, direction-specific thresholds,
  seed/hop/fan-out/visited/time caps and digests before G08 final freeze. B remains R4
  proofs only; descriptor lookup is a declared dependency, not hidden passages.
  Semantic linking may remain off on poor gain/cost. D/E development requests
  are additional to the 2,880 mandatory acceptance requests, never substituted.

## Phase 5: User Story 4 — freeze measurement early

### G07 [US4] Draft the private graph questions

**Time:** 4 h. **After:** G05, G06, G34; D7-approved suite size and recorded
receipt for the approved whole published `ctm` generation, local-only.
**Files:** `PRIVATE/evals/ctm/ctm-graph.jsonl`,
`PRIVATE/evals/ctm/ctm-graph-splits.json`,
`PRIVATE/evals/ctm/ctm-graph-manifest.json`,
`PRIVATE/graph/receipts/acceptance-inputs.json`.
**Test:** `maestro eval graph draft --manifest "$PRIVATE/evals/ctm/ctm-graph-manifest.json"`
then `maestro eval graph check --manifest "$PRIVATE/evals/ctm/ctm-graph-manifest.json"`,
with plan Validation's scratch environment and default-kernel refusal check;
standard output/error contain only aggregates, IDs, digests and fixed codes.
**Acceptance:** FR-S2-018, FR-S2-021, FR-S2-024; approved 220 FR/EN questions,
50 per answerable class plus 20 unanswerable; all held out, 200 answerable
independent families. Pilot/synthetic development cases stay separate.
**Executor / inference:** lane-run Maestro commands expose aggregate metrics,
IDs and digests only. Drafting/source reads use local models through the router,
never hosted context; G08's different-local-family review and owner local flag
rulings precede freeze.

- [ ] **Red.** Reject missing anchors, wrong quotes, duplicate acceptance
  families, overlap with pilot/synthetic development and missing complete proof.
  Reject pilot-only permission, missing scope/target/expiry/evidence fields,
  expired approval and missing/overlapping scratch bindings. Prove the check
  never opens the default kernel, using G06's synthetic refusal test.
- [ ] **Green.** Record the 12:53 whole-published-generation/local-only approval
  and restore the named backup under plan Validation. Freeze generation/document/
  version inventory, digests and window policy in `acceptance-inputs.json`.
  Freeze the passage-only route/weight/reranker definition, then use G34's
  local drafting runner to write fifty questions per answerable class and twenty
  unanswerable directly to PRIVATE, stratified by EN/FR and exact/paraphrased/
  cross-language linking without duplicate translated families. All 220 stay acceptance-only; no label/failure guides tuning. Label truth,
  conditions and alternate chains; require one independent family per item.
  Extra source versions need approval.
- [ ] **Check.** Run Test; read only its counts/IDs/digests, never draft text.
  G06's local checker verifies languages, anchors, unanswerable rationales and
  FR-S2-024's supported vocabulary subsets. Missing
  source-backed coverage blocks freeze, not permission to invent claims.
  Check documentary rather than customer-instance semantics; do not add `Job`,
  misuse event satisfaction or count C27a catalog edges as documentary proofs. Hand
  off the draft digest for G08 without calling an unreviewed set a golden set.
  G07 consumes frozen source/arm-A inputs, not G38 output, so drafting may
  overlap development. G08 alone consumes G38's selected C/linking/extractor
  profile for final suite freeze; drafts/results never guide G38 tuning.

### G08 [US4] Review every chain and record the S1 baseline

**Time:** 4 h. **After:** G05, G07, G32, G33, G38; integrated S1 search/ask, eligible local card and
owner-confirmed local reviewer protocol.
**Files:** `PRIVATE/evals/ctm/ctm-graph-review.jsonl`,
`PRIVATE/evals/ctm/ctm-graph-manifest.json`,
`PRIVATE/graph/receipts/baseline.json`, `PRIVATE/graph/receipts/baseline.md`.
**Test:** `maestro eval ladder --manifest "$PRIVATE/evals/ctm/ctm-graph-manifest.json"`,
with plan Validation's scratch environment.
**Acceptance:** FR-S2-018, FR-S2-021, SC-S2-002, SC-S2-004; frozen reviewed labels,
profile/input digests and S1 baseline with a fixed at-most-4B answerer.
**Executor / inference:** lane-run Maestro commands expose aggregate metrics,
IDs and digests only; a different local model family from the router catalog
reviews every chain and the owner rules on flags on a local review page.
Answering runs locally through the router; no private text enters hosted context.

- [ ] **Red.** Refuse a freeze with an unreviewed chain, unresolved flagged
  wording/answerability change, changed input digest or missing failed attempt.
- [ ] **Green.** Restore the named backup into the isolated scratch kernel;
  validate private Qdrant bindings and never use the live kernel. The pinned
  G32 runner checks every chain locally; G33's page/import records owner flag
  rulings. Only aggregate metrics, IDs and digests reach either output channel.
  Freeze source/suite and passage-only route/weight/reranker digests plus the
  S1 default at-most-4B answerer card by digest, including its
  current reasoning/sampling settings. Freeze G38's descriptor thresholds/caps,
  passage bounds and C-route choice, S1 embedding/reranker/weights, shared context/
  wire budgets and the single 20 s end-to-end deadline. Record S1 as drift only.
  Require G06's power table and D7 count before freeze; no post-result resizing.
- [ ] **Check.** Run Test on that scratch restore, retaining every failure.
  Neither the held-out labels nor baseline failures may guide tuning. G23 uses
  this exact answerer card but compares gain with its same-run passage-only rung.
  Freeze graph 200-answerable/20-unanswerable and unchanged golden 84/16 scoring,
  including golden answer/refusal gates and all 2,880 answered requests. Record
  semantic-link trigger frequency and cost even for golden simple lookups.

### G20 [US4] Compare rules with rules plus the 4B extractor

**Time:** 3 h. **After:** G05, G08, G19, G28, G32, G33; approved acceptance scope
and development-only 8B trigger, owner-confirmed local precision reviewer.
**Files:** `PRIVATE/graph/receipts/extractor-manifest.json`,
`PRIVATE/graph/receipts/extractor-attempts.jsonl`,
`PRIVATE/graph/receipts/extractor-comparison.md`,
`PRIVATE/graph/receipts/extractor-card.json`,
`PRIVATE/graph/receipts/m2-graph.json`.
**Test:** `maestro eval ladder --manifest "$PRIVATE/graph/receipts/extractor-manifest.json"`,
with plan Validation's scratch environment and G06's construction scorer.
**Acceptance:** SC-S2-003, FR-S2-016, FR-S2-018, FR-S2-021; three retained runs with
independent-model semantic precision/recall, owner rulings on flags, quote
validity and tokens/throughput/VRAM. The review protocol is pending owner confirmation.
**Executor / inference:** lane-run Maestro commands expose aggregate metrics,
IDs and digests only; extraction runs locally through the router. A different
local model family from the router catalog reviews every claim, including
rule-made claims; owner flag rulings use a local review page. No private text
enters hosted context.

- [ ] **Red.** Refuse comparison when suite/profile/corpus digests differ,
  failed attempts are missing, synthetic evidence is called real or a candidate
  violates the precision floor despite exact quotes. Reject missing graph
  selection rules, graph digest mismatch and extractor selection on held-out data.
- [ ] **Green.** Before acceptance use, verify G38's `extractor-card.json`,
  exact asset/card/licence digests and GPU cost evidence (D2, ENF-012); missing
  or changed receipts refuse rather than recording an asset after first use.
  Restore the named
  backup with private Qdrant bindings; never open the live kernel. Verify G38's
  preregistered development gold set/trigger and selected-card receipt. Record
  the first-complete-run graph selection rule before acceptance extraction;
  do not reselect the frozen card on held-out data. I1's approved 8B trigger is 4B precision <95%
  or gold-claim recall <80% on pilot/synthetic development only; a trial still
  needs the recorded failed-4B evidence and revised download/asset estimate.
  Compare rules with rules-plus-selected-card on the approved acceptance scope.
  G32 reviews every accepted held-out claim locally; G33 records owner flag
  rulings. Only its sanitized summaries reach the lane; all three precision
  point estimates must reach 95%.
- [ ] **Check.** Run Test; retain every rejection/failed attempt. Under the
  preregistered rule, use the development-selected card's first complete run
  and frozen rules/profile, not the best acceptance result. Attach that claim
  set in scratch, build with G28, verify G27 readiness, then freeze the M2 graph
  in `m2-graph.json`: collection/generation, source/profile/card/claim-set and
  projection-content digests plus a named authoritative scratch backup receipt.
  Include selected descriptor/transition builder/embedding/profile/content
  digests from G38. The same shared 20 s request deadline and S1 profiles carry
  into acceptance; extraction budgets remain separately bounded offline work.
  A failed gate blocks; do not choose a later graph. G23 reuses this graph for
  every rung/repeat; never re-extract for repeated stochastic measurements.

## Phase 6: User Story 5 — recovery and native proof

### G21 [US5] Consume projection events durably in the foreground

**Time:** 4 h. **After:** G09, G19, G25, G28; G19's shared build-command edit lands first.
**Files:** `G/consumer.rs`, `G/tests/consumer.rs`,
`K/src/telemetry/span.rs`, `K/src/telemetry/tests/stages.rs`,
`C/src/cli/graph/build.rs`, `C/tests/it/graph_consumer.rs`.
**Test:** `capped cargo nextest run -p maestro-knowledge graph::tests::consumer`
and `capped cargo nextest run -p maestro graph_consumer`.
**Acceptance:** FR-S2-020; S1 cursors drive idempotent durable projection
and local telemetry effects, without a daemon or OTLP dependency.

- [ ] **Red.** Kill before effect, after effect and before ack; test replay,
  lag, hidden scopes, redaction and loss of graph files after cursor advancement.
- [ ] **Green.** Foreground catch-up schedules a full G28 build for each
  newly attached generation, not an incremental loader. Deduplicate receipts
  and ack only durable build effects. Lost files rebuild the frozen snapshot,
  regardless of cursor advancement; never trust only the tail.
- [ ] **Check.** Run both Test commands; a restart neither duplicates effects
  nor skips missing projection content. Telemetry contains no quotes/prompts or
  hidden-source identifiers and is never the authoritative progress record.

### G30 [US5] Prove deletion and rebuild produce identical results

**Time:** 4 h. **After:** G11, G13, G28, G21, G35, G36.
**Files:** `N/tests/it/ladybug_rebuild.rs`,
`N/tests/it/ladybug_rebuild/fixture.rs`, `N/tests/it/ladybug_rebuild/compare.rs`,
`N/tests/it/ladybug_rebuild/processes.rs`, `S2/research.md`.
**Test:** `capped cargo nextest run -p maestro-knowledge --test it ladybug_rebuild`.
**Acceptance:** FR-S2-019, SC-S2-006; actual disposable-file deletion and exact
ordered semantic equality, not only equal node counts or query success.

- [ ] **Red.** Freeze kernel/queries and save neighbors, paths, claims,
  supports, evidence and coverage. Ensure the oracle detects reordered paths,
  changed quote/span/conditions, missing contradictions and wrong generations.
- [ ] **Green.** Drain readers/close handles, delete only owned disposable
  LadybugDB files and rebuild from SQLite/artifacts. Compare with the saved
  results, excluding only timing/transport IDs. Repeat G28 after interrupted
  batches, not a second loader. Delete and rebuild G35's descriptor collection
  and G36's transitions too; compare canonical descriptor text/IDs/disjoint
  pointers/scope payloads/profile digests and deterministic lookup/transition
  fixtures, not approximate ANN ordering. Graph-file/transition rebuild needs
  no model; only descriptor re-embedding may use the pinned embedding profile.
  Use no graph/descriptor backup or Qdrant authority.
- [ ] **Check.** Run Test across collections, retained generations, hidden
  hops and version conditions. Prove deletion really occurred and other
  collections/kernel/artifacts survived; record process/pin evidence.

### G22 [US3, US5] Restore the authority and repeat the native drill

**Time:** 4 h initial + 1 h cross-slice follow-up (5 h total).
**After:** G14, G15, G16, G19, G30, C48; existing release inputs remain required.
**Files:** `C/tests/it/graph_restore.rs`,
`C/tests/it/graph_processes.rs`, `C/src/cli/backup/restore.rs`,
`C/src/failure.rs`, `K/src/store/error.rs`, `K/src/store/tests/migrations.rs`,
`N/tests/it/ladybug_rebuild.rs`, `.github/workflows/integration.yml`,
`.github/workflows/lbug-qualification.yml`.
**Test:** `capped cargo nextest run -p maestro graph_restore` and
`capped cargo nextest run -p maestro graph_processes`.
**Acceptance:** FR-S2-019, SC-S2-006, SC-S2-007; SQLite/artifact restore →
G30 rebuild → evidence equality, plus native writer/CLI/MCP process CI.

- [ ] **Red.** Test missing/corrupt artifacts, partial restore, denied
  supports, old pins and interrupted builds. Retained S1 must refuse an S2
  scratch database with `UnknownMigration` and no mutation. A graph alone
  cannot repair authority; test current kernel/CLI version-mismatch guidance.
- [ ] **Green.** Add native Linux/Windows/macOS process cases to the core
  workflow/drill. Its recovery receipt and diagnostic name the next action:
  update the binary or restore the named pre-upgrade backup. Add that guidance
  to current errors; do not claim the historical S1 executable prints it.
  Reusable toolchain release/re-pin waits belong to the supervisor.
  Build/package M2 with `cargo build -p maestro --release --locked --features engine`;
  default featureless builds remain separately checked. Retain NOTICE, qualified
  combined-pin and default/feature coverage plus complete mutation-union evidence.
- [ ] **Check.** Run both Test commands plus
  `capped cargo nextest run -p maestro-kernel store::tests::migrations`.
  Observe native CI, crash/reader-drain evidence and unchanged authority after
  S1 refusal. Delete descriptor/transition projections after authority restore
  and require G30's deterministic equality without a projection backup.
  Queued jobs are not passes; public CI uses no vendor fixtures.

## Phase 7: User Story 4 — M2 acceptance, then release

### G23 [US4] [P] Measure the three retrieval variants against fixed gates

**Time:** 3 h active lane work; acceptance runtime/review waits are separate.
**After:** G05, G08, G16, G20, G32, G33, G38; D7-approved suite, local
reviewer protocol and acceptance scope.
**Files:** `PRIVATE/graph/receipts/m2-manifest.json`,
`PRIVATE/graph/receipts/m2-attempts.jsonl`,
`PRIVATE/graph/receipts/m2-comparison.md`,
`PRIVATE/graph/receipts/ctm-retrieval-regression.json`.
**Test:** `maestro eval ladder --manifest "$PRIVATE/graph/receipts/m2-manifest.json"`,
with plan Validation's scratch environment.
**Acceptance:** FR-S2-018, FR-S2-021, SC-S2-002, SC-S2-003, SC-S2-004, SC-S2-005;
passage-only, LadybugDB-only and pairing on frozen inputs without duplicate
embeddings. Every D3 floor is a gate, not a report-only aspiration.
**Executor / inference:** lane-run Maestro commands expose aggregate metrics,
IDs and digests only; answering runs locally through the router. Claim review
uses the pinned different local model family from its catalog, with owner flag
rulings on a local review page. No private text enters hosted context.

- [ ] **Red.** Refuse mismatched generator/card/context/sampling/suite/graph
  digests, missing G20 graph receipts, omitted failures and held-out tuning.
  Keep cold, warm and unavailable latency cohorts separate.
- [ ] **Green.** Restore G20's named digest-frozen authoritative scratch backup
  containing the M2 claims; validate private Qdrant bindings. Verify/rebuild
  exactly that frozen projection through G28 and match its semantic digests,
  not an empty S1-only graph. Run every rung on this same binary/input graph.
  Use G08's frozen at-most-4B card/settings; repeat retrieval/answer stochastic
  work three times, never re-extract. Keep every attempt. Compare pairing with
  same-run passage-only (S1 default routes/weights/reranker, graph `none`);
  G08 is drift only. Answer all 220 graph and unchanged 100 golden questions
  on every same-run rung/repeat: 1,980 + 900 = 2,880 mandatory requests. C uses
  G38's frozen route definition; B declares descriptor dependency but no passage
  retrieval. Retain route attribution and golden trigger frequency/lookup cost.
  G32/G33 supply any review/ruling evidence; all commands write raw reports
  privately and expose only aggregates, IDs and digests, including on failure.
- [ ] **Check.** Run Test; every run must independently pass: ≥5-point proof
  gain measured at the delivered evidence bundle after context/wire budgets,
  with the seed-0/2,000-resample 50th delta strictly above zero; no `ctm-graph`
  Recall@10/MRR@10/supported-answer point-estimate loss; no `ctm-retrieval`
  Recall@10/MRR@10 point delta below zero (seeded intervals diagnostic only);
  golden supported-answer rate on 84 entries and correct refusal on all 16
  unanswerables each ≥ same-run A. Errors/timeouts earn no support/refusal credit;
  relation precision ≥95%, exact spans/quotes/commands and ≥16/20 refusals.
  Warm private p95: graph span/tools ≤500 ms, search <2.5 s, ask <10 s.
  Every request has one 20 s end-to-end deadline including queues/loading/retry;
  cancellation and failed attempts stay in the predeclared cohort. Report
  proof attrition, exact citation coverage and unsupported conclusions separately.
  Missing, failing or inconclusive evidence blocks M2; negative retrieval
  deltas disable losing fusion. Do not average away a failure or retune.

### G24 [US1, US2, US3, US4, US5] Map evidence and release M2

**Time:** 3 h. **After:** G23; every other task integrated and CI fixes complete.
**Files:** `S2/acceptance.md`, `S2/spec.md`, `S2/plan.md`, `S2/tasks.md`,
`S2/research.md`, `CONTEXT.md`, `docs/architecture/README.md`,
`docs/architecture/02-retrieval-and-knowledge-graph.md`,
`docs/architecture/04-intelligence-backend.md`,
`docs/architecture/05-platform-and-operations.md`,
`docs/architecture/06-roadmap.md`, `docs/architecture/08-traceability.md`,
`docs/how-to/knowledge-graph.md`,
`docs/adr/0021-embedded-ladybug-graph-projection.md`,
`docs/adr/0004-neo4j-for-the-graph-projection.md`,
`docs/adr/0002-one-authority-many-projections.md`,
`docs/adr/0020-rust-libraries-with-named-dependency-exceptions.md`, `docs/adr/README.md`.
**Test:** requirement-by-requirement evidence check plus the final CI gate and
synthetic Pi/Claude Code smoke commands in `docs/how-to/knowledge-graph.md`.
**Acceptance:** SC-S2-001, SC-S2-002, SC-S2-003, SC-S2-004, SC-S2-005,
SC-S2-006, SC-S2-007, SC-S2-008; supervisor-approved release evidence.

- [ ] **Red.** Acceptance check rejects a missing requirement, duplicate map,
  pending/failed G25 or native run, stale commit receipt, lowered target,
  unproved equality, unmapped rule ID or any unmet M2 exit. Design status is not
  code evidence. G25's Check records the interim rule-table recheck against its
  qualification verdict; this task owns the final pre-acceptance recheck.
- [ ] **Green.** Map every FR-S2/SC-S2 and its architecture portion to an
  integrated commit/test/receipt or an explicit blocker; keep private content
  and receipt bodies private. Record packaging, process, rebuild and client
  evidence and remaining named deferrals. Finalize ADR-0021 against G25's
  actual evidence here, not in G01. Reconcile CONTEXT's projection/graph terms
  and architecture index/status rows with that evidence. The supervisor opens
  a separate organization `.github` PR for the constitution's stack row once
  ADR-0021 is final; this core task does not edit another repository.
- [ ] **Check.** Recheck the ID-keyed Constitution Check, final CI
  coverage/mutation/three-OS gates and G25
  against the release commit. Owner authorizes live migration only after a
  named backup and the `maestro-s1`/client update. Obtain supervisor release
  approval; lanes never merge, deploy or upgrade the owner's kernel.

## Dependencies and parallel opportunities

The **After** line is authoritative for G tasks and G28a–G28d; the E table
owns G27 slice prerequisites. Every G predecessor appears earlier above.
C44 → G01, C46 → G26 and C47a → E07a/G28 are explicit cross-slice waits; external
private/CI evidence is separate. M1 release is not an S2 start gate.
G16 additionally waits for G15 so answer validation cannot outrun the
whole-proof transport contract; G30 waits for G13 so its equality oracle can
compare assembled graph evidence. G24 waits for every S2 task/follow-up, not
only the quality report.

For the combined DAG, G27's native umbrella completes only after E11.
G28 consumes its qualified native lifecycle through E10 and may start before
E11's receipts/how-to (2026-10-03 13:33 ruling); G24 still requires both complete
umbrellas. G25 requalifies the combined E01/E02/E03/E03b fork before E07a.
The S3 handoff is **G25/E07a qualification → C48 → G22 → C49a**, never C48 ↔ G22.
C49a also joins E11/G28 and its existing C47b/C48/C18/C16b inputs. E11 supplies
engine/gate receipts; G22 performs release packaging/drills, not a prerequisite
for E11 or C48. C49a is a downstream S3 consumer, not an added S2 release gate.

| Once these land | Independent work, subject to shared-file ownership |
| --- | --- |
| G01, after C44 | G02 claims and G17 model role start beside G25; G26 also needs G25 and C46. |
| G03 | G06 scoring, G09 builds and G31 vocabulary are `[P]`; shared registration hunks are supervisor-rebased, not concurrent writes. G10 waits for G09/G31 so resolver edits serialize too. |
| G06 | G34 local drafting tooling can run beside public native work; actual private drafting still waits for G05 and the frozen receipt. |
| G04 and G34 | G32 reviewer/capture, then G33 owner page, unblock G05; G11/G12 proof work can proceed beside these tools. G34 precedes G32 for their shared manifest. |
| G28 and G25 | G04 pilot neighbors can start immediately (G03 already landed); G11 adds paths after G04. |
| G05 and G34 | G07 drafts source/arm-A questions independently of G38; G08 final freeze additionally waits for G38 selection, recorded receipts and reviewer confirmation. |
| G27's qualified native lifecycle through E10, G35, G26 and C47a | G28a → G28b → G28c → G28d can proceed without E11; no slice alone closes G28. S3 C27a separately consumes the qualified G27 port; coordinate its public contract. |
| G13 and G35 | G14 R4 and G15 tools proceed with a shared contract; G36 follows G11, G37 joins both routes, then G16/G38. |

These are opportunities, not permission to write shared files in parallel.
The supervisor assigns at most five building lanes under the workspace agent cap and owns common module, manifest,
lockfile, migration and CLI-registration conflicts. G09/G31 → G10 and
G34 → G32 serialize the migration/resolver and private-manifest work;
other migration/CLI registration hunks are supervisor-rebased exceptions, not
permission to race shared edits. G06 waits for G03, not the native pilot.
G19's shared build-command edit precedes G21's (explicit dependency).
Model/GPU measurements and writers serialize on their actual resources. Each task is one reviewable
behavior, not a framework assembled before the first useful result.

### Recomputed effort and dependency accounting

The G-heading inventory remains **37 tasks** (G01–G38 except removed G29),
including G27 and G28 umbrellas. The **35** other initial budgets total
**132 h**; five reopened follow-ups add **9 h**, and G01/G26/G22 cross-slice
follow-ups add **3 h**, for **144 h**. G28's **four slices** add
**2 + 4 + 4 + 3 = 13 h**, including its original cross-slice hour, giving
**157 h** bounded G work. This replaces its former 5 h by 13 h (**+8 h**), not
scope or acceptance, per the 2026-10-03 16:38 ruling. G27's **14 E slices**
replace its old four-hour allocation; E07a **+2 h** and E08b **+1 h** remain
inside the **43–89 h** E ranges.
Thus **157 + (43–89) = 200–246 lane-hours**, not elapsed time or remaining work.
The original cross-slice increase stays **4 + 2 + 1 = 7 h**; G28's included
hour is not added again. Already landed base tasks/slices remain in this
inventory, but their new deltas are explicitly budgeted and dispatched
separately. Extraction, acceptance runtime, human review and external native
CI/gate-release waits remain additional.

For a reproducible bounded-G subtotal only, give G27 weight zero, use G28's
13 h serial slice total once, add other reopened/cross-slice G deltas to their
named tasks, use internal G **After** edges and make G24 wait for all G tasks/
follow-ups. Treat all C/E boundaries as already available for this calculation
only; it is not the combined release path.
The longest weighted G path is **80 h**:
G01 → G02 → G03 → G31 → G10 → G35 → G28a → G28b → G28c → G28d → G04 →
G11 → G13 → G14 → G37 → G16 → G38 → G08 → G20 → G23 → G24.
G31's follow-up breaks its former tie with G09.
The pilot's G ancestor set costs **78 h** with a **55 h** longest G path:
G01 → G02 → G03 → G31 → G10 → G35 → G28a → G28b → G28c → G28d → G04 →
G32 → G33 → G05.
These full-plan subtotals include base work plus all G deltas; they neither
measure remaining work nor waive the G10/G35 recheck or native qualification.
G38 step 0 stays inside its 4 h budget; no early-handoff overlap credit is taken.

For scheduling, expand G27 into its E prerequisites and E11 completion join,
let G28 start on the qualified native lifecycle before that join, retain
C44/C46/C47a/C48 and the downstream C49a join, and use actual input and
completion receipts at every **After** boundary. A single G27-duration formula
cannot represent the new independent C waits. Fork/E05/E06 work can overlap
upstream G work, E09/E10 can overlap, and qualification/re-pin waits are
external. No combined critical-path duration or completion date is claimed.

## Requirement coverage

Generated from each task's **Acceptance** line, not inferred extra ownership.
These are planned owners, not delivered statuses. G01 checks exact equality in
`crates/maestro-knowledge/tests/it/graph_fixture.rs`; missing, extra and duplicate
map rows fail. Owners stay in task/document order, not numeric order (for
example `G28, G30, G22`); only `### GNN [US…]` headings define tasks.
`#### G28a` through `#### G28d` inherit the G28 umbrella requirements without
adding duplicate ownership rows or independent acceptance. G24 replaces
planning claims with integrated evidence/blockers before release.

| Requirement | Tasks |
| --- | --- |
| FR-S2-001 | G25, G27 |
| FR-S2-002 | G02, G31, G10 |
| FR-S2-003 | G02, G03, G31, G19 |
| FR-S2-004 | G01, G03 |
| FR-S2-005 | G09, G19 |
| FR-S2-006 | G10 |
| FR-S2-007 | G27, G04, G36 |
| FR-S2-008 | G27, G35, G28, G36 |
| FR-S2-009 | G11, G36, G14 |
| FR-S2-010 | G04, G11, G36, G14, G15 |
| FR-S2-011 | G35, G14, G38 |
| FR-S2-012 | G12 |
| FR-S2-013 | G06, G13, G15, G37, G16 |
| FR-S2-014 | G04, G15 |
| FR-S2-015 | G06, G16 |
| FR-S2-016 | G17, G18, G38, G20 |
| FR-S2-017 | G19 |
| FR-S2-018 | G06, G34, G32, G33, G38, G07, G08, G20, G23 |
| FR-S2-019 | G35, G28, G30, G22 |
| FR-S2-020 | G21 |
| FR-S2-021 | G01, G34, G32, G33, G05, G38, G07, G08, G20, G23 |
| FR-S2-022 | G26, G27 |
| FR-S2-023 | G27, G36 |
| FR-S2-024 | G31, G18, G19, G07 |
| FR-S2-025 | G36, G37, G38 |
| SC-S2-001 | G01, G04, G32, G33, G05, G24 |
| SC-S2-002 | G06, G14, G38, G08, G23, G24 |
| SC-S2-003 | G06, G32, G33, G16, G20, G23, G24 |
| SC-S2-004 | G06, G16, G38, G08, G23, G24 |
| SC-S2-005 | G11, G14, G37, G16, G38, G23, G24 |
| SC-S2-006 | G35, G28, G36, G30, G22, G24 |
| SC-S2-007 | G25, G27, G22, G24 |
| SC-S2-008 | G15, G24 |

## Minimal-first delivery strategy

1. Run qualification beside public construction; pilot graph reads use LadybugDB.
2. Freeze measurement before tuning; finish immutable claims and safe files.
3. Extend LadybugDB neighbors with paths and whole proofs before graph answers.
4. Add only bounded offline extraction, then prove recovery and measured gain.
5. Release only on the evidence map and fixed M2 gates. Qdrant Edge, fuzzy
   identity merging, communities, general calibration and other slices stay separate.
