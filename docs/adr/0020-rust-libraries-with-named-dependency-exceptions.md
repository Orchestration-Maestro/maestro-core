# Rust libraries join the stack; the duplicates they force are named exceptions

Status: accepted, 2026-09-26; S6 browser-role amendment accepted, 2026-09-30.

The S6 amendment below supersedes the original acquisition/conversion shortlist
where it differs. Approval of a direction is not dependency adoption or platform
qualification; this amendment changes no workspace dependency or policy file.

Maestro builds on the Rust ecosystem's libraries rather than on hand-written
clients: `qdrant-client` for the search projections (ADR-0003), `neo4rs` for
the graph projection (ADR-0004), `docling` (docling.rs) for document
conversion, `spider` and `dom_smoothie` for acquisition. The organization's
one-version rule (DEP-001) stays the default. The ecosystem is midway through
several major transitions (`syn` 2 to 3, `getrandom` 0.2 to 0.3,
`windows-sys`), so an adopted library can force a second version of a crate
maestro-core already uses. Each such duplicate is a DEP-001 exception in
`maestro-quality.toml` naming the crate and version, the library that forces
it and the version that ends it, and it is removed when the library moves.

Before a library is adopted, its tree is measured against the workspace's
lock: the crates it adds, the second versions it forces, the native libraries
it links. It is taken with the fewest features it needs, and the numbers go
into the task's brief. A library whose native link clashes with the core's is
taken only with features that avoid the clash.

Measured on 2026-09-26 against maestro-core's 128 crates:

| Library | Crates in its tree | New to maestro-core | Second versions it forces |
| --- | ---: | ---: | --- |
| `qdrant-client` 1.19.0, no default features | 128 | 78 | 4: `syn`, `base64`, `getrandom`, `windows-sys` |
| `neo4rs` 0.9.0-rc.10 | 122 | 84 | 4: `syn`, `getrandom`, `socket2`, `windows-sys` |
| `dom_smoothie` 0.18.2 | 61 | 45 | 1: `syn` |
| `docling` 1.69.2, no default features to defaults | 146 to 439 | 120 to 363 | 9 to 23 |
| `spider` 2.53.9, no default features to defaults | 268 to 424 | 186 to 331 | 15 to 38, a second `reqwest` among them |

`spider`'s default features also link a second SQLite (`libsqlite3-sys` 0.30
beside the kernel's 0.38), which no build can link; without them it links
none.

## S6 amendment: crawl4ai browser-render-only exception

**Owner decision, 2026-09-30:** Rust owns acquisition producers, connectors and
extraction. The sole Python exception is **crawl4ai, out of process**, only where
JavaScript or Chromium is unavoidable, behind the replaceable admitted fetch/render
port. It replaces Spider's chromey route; Spider remains the Rust crawler with
`default-features = false`, extras, `chrome` and `chromey` off. No chromey,
chromiumoxide, WebDriver or other production browser fallback is approved.

The exception permits bounded browser requests/rendered DOM capture only:
**no frontier** or independent crawl/discovery authority, **no conversion** (including
Markdown extraction), **no model** execution/download, and **no publication** or
indexing role. Rust core policy admits every destination and validates returned
captures. Configured readiness is bounded and declarative, not arbitrary source
scripts or network-idle alone. A disabled, missing or unqualified adapter refuses;
its callers do not install or select another browser stack.

### Artifact and licence record

N02 inspected the public **crawl4ai 0.9.4** release, not an installed deployment.
The candidate PyPI wheel `crawl4ai-0.9.4-py3-none-any.whl` was downloaded only
for archive inspection, not installed; its verified SHA-256 matches the registry:
`46779c8a93b35c7c5f9be9642ddef5e7b515e36b094639bb8e1e418bf0266179`.
The source distribution has registry SHA-256
`fd118861b1acff90ac0db4ceae4d529691fd1a0a0d4417c710de3c7d2114ae88`.
These identify audit candidates, not approved production artifacts. Before launch,
the adapter declaration must bind exact adapter/wheel, Python interpreter,
transitive wheels/native assets, browser automation runtime and Chromium build
versions/digests, protocol, configuration, supported OS/architecture and licence
receipts. PyPI's `Python >=3.10`, `playwright >=1.49.0` and
`patchright >=1.49.0` are **not closure pins**. The exact Python/browser closure,
ABI compatibility and platform evidence remain blockers; no version is invented.

PyPI reports Apache-2.0, but the [release licence](https://github.com/unclecode/crawl4ai/blob/v0.9.4/LICENSE)
also has an **Attribution Requirement**. The fetched text and the wheel's embedded licence both have SHA-256
`193fe32704bee15cd74f5153e569cdf830e26445e19798a55daee32da40758aa`.
It requires this notice in a prominent, accessible location, including CLI help:

> This product includes software developed by UncleCode (<https://x.com/unclecode>)
> as part of the Crawl4AI project (<https://github.com/unclecode/crawl4ai>).

Review the actual wheel/source terms, notice placement and every Python/native/
browser component before distribution or use. The SPDX label alone does not
settle the additional terms; that review remains a blocker. The mandatory Python
requirements include extraction/LLM packages even with extras off; their presence
grants no conversion/model capability and their full closure still needs audit.

### Controls, related shortlist and removal

All adapter/runtime/browser assets are provisioned in advance, pinned and verified;
**no downloads** at build, launch, first use or recovery. Do not invoke
`crawl4ai-setup`, browser installers, model-download commands or the upstream live
`doctor` probe. Upstream setup installs both Playwright and Patchright Chromium;
it is not the qualified single-route launcher. Browser traffic is allowed only
through current core admission, including redirects/subresources and otherwise
bypassing channels. N46 must prove checked-address egress, credential isolation,
sandbox/resource bounds, readiness, cancellation and owned-process-tree reaping
on each supported platform; process separation alone is not containment. Missing
controls hold that platform/adapter rather than enabling an unsafe fallback.

The approved bake-off is unchanged: **Xberg 1.3.0** minimal native PDF/Rust
layout/OCR versus native **docling 1.78.0**, no defaults plus `pdf-text`.
Python Docling remains comparison-only. **tract is the layout/OCR default;
dynamic ONNX Runtime is table-only**, with offline, digest-pinned, licence-checked
weights/runtime assets. Never enable `download-binaries`: the published Xberg
ORT and docling ML feature edges still require an upstream split or an explicitly
approved audited patch. Enabling dynamic loading does not subtract download
features. Optional native/model exceptions, unselected libraries and every patch
remain individually owner-approval-blocked.

Remove this Python exception when a **qualified Rust browser adapter** meets the
same transport, readiness, session, containment, offline-artifact and three-platform
controls, and the owner approves its measured dependency/artifact record. Replace
the adapter behind the port, remove the Python/browser closure that is no longer
needed, and retire the exception with its qualification evidence. Until then,
no alternative library is implicitly approved.

N02's [current-lock measurements and blockers](../../specs/006-native-acquisition/research.md#39-n02-measured-dependency-probe)
use S6 integration `e16cad0f82e329d6a3971379078cc2560d00d7d8`: **229 packages,
221 registry packages**. The 128-crate table above remains explicitly historical.
Every later addition repeats the probe against its then-current lock and records
each forcing library/version, licence, native link and duplicate-removal condition
before any DEP-001/vet record or dependency adoption.

## Considered options

- Hand-written HTTP clients, T026's first design: no duplicates, but maestro
  would maintain protocol code the vendors already maintain, for every
  service. Rejected.
- Relaxing DEP-001 for the whole organization, or for build-only crates such
  as `syn`: fewer exceptions, but the rule's readers would no longer see which
  library holds which duplicate. The build-only case, procedural-macro
  dependencies that ship in no binary, is worth proposing to the gate's
  maintainers; until the gate distinguishes it, exceptions.
- Every large library in its own process, ADR-0013's extension model: it
  isolates crashes and untrusted input at the cost of a protocol per library.
  Kept as a choice each adoption weighs, for example a crawler or a PDF parser
  reading untrusted input, not as a rule.

## Consequences

`maestro-quality.toml` gains an exception for each forced duplicate, with its
removal condition, and `cargo vet` gains an exemption or an audit for each new
crate. Builds grow with each library, since a second `syn` compiles twice;
binaries grow less, since build-only duplicates ship in none. Upgrading a
library is a task: measure again and drop the exceptions it no longer forces.
