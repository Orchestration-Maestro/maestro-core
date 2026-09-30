# C09 dependency measurements

**Measured:** 2026-09-29, on `feat/s3-c09` at `b2f3b7f`. No dependencies
were added to the workspace or lockfile. Scratch manifest and generated lock:
`/tmp/c09-measure/Cargo.toml` and `/tmp/c09-measure/Cargo.lock`.

## Selection and features

The D4-approved candidates are measured independently in a scratch package:

```toml
[dependencies]
tar = { version = "0.4", default-features = false }
jsonschema = { version = "0.30", default-features = false }
cedar-policy = { version = "=4.13.0", default-features = false }
```

Resolved versions were `tar 0.4.46`, `jsonschema 0.30.0`, and
`cedar-policy 4.13.0`. `jsonschema` 0.30 implements draft 2020-12; the
resolver features are disabled so catalog schemas cannot trigger remote or
local reference retrieval. This is the minimum measured selection for schema
validation. `tar` with default features disabled does not enable `xattr`; the
bundle format needs file bytes and paths, not extended attributes.

`cedar-policy`'s public package resolves with no enabled package features, but
its 4.13.0 manifest depends on `cedar-policy-core = 4.13.0` without disabling
default features. Consequently the core still enables `datetime`, `decimal`
and `ipaddr`; these are unavoidable through the approved public crate's current
manifest, not selected by this consumer. `experimental`, protobufs, wasm,
async, and heap profiling were not enabled.

## Closure and platform costs

Evidence commands used scratch and current-workspace metadata/tree outputs;
all `cargo` invocations used `~/.local/bin/capped`, `CARGO_BUILD_JOBS=3`, and
`CARGO_INCREMENTAL=0`.

- Scratch resolved closure: 216 packages, versus 230 for the current workspace
  resolution. Comparing the scratch lock's registry packages with the current
  workspace registry package set yields 138 new name/version pairs (the
  scratch closure includes separate versions of shared crates as well as its
  own unique crates). This is a worst-case addition count, not the eventual
  lockfile delta: Cargo unifies compatible versions when adopted in workspace.
- Existing workspace has duplicate versions of `base64` (0.22.1/0.23.1),
  `getrandom` (0.2.17/0.4.3), `hashbrown` (0.15.5/0.17.1), and `syn`
  (2.0.119/3.0.6). The scratch closure has `itertools` (0.14.0/0.15.0),
  `syn` (2.0.119/3.0.6), and `unicode-width` (0.1.14/0.2.2). Thus `syn` is
  already duplicated; `itertools` and `unicode-width` are additional duplicate
  version pairs in the candidate closure. Check the integrated lock before
  adopting and record DEP-001 exceptions only for actual retained duplication.
- Active Cargo `links` metadata in the current workspace: `ring`,
  `libsqlite3-sys`, and `wasm-bindgen-shared`. Scratch: `defmt` and
  `wasm-bindgen-shared`; no C library/native system dependency is introduced by
  these selected features. `defmt` is a Rust link namespace, not a native C
  library. The scratch manifest's feature-disabled `tar` has no `xattr` crate.
- License metadata: `tar 0.4.46` is `MIT OR Apache-2.0`; `jsonschema 0.30.0`
  is `MIT`; `cedar-policy 4.13.0` is `Apache-2.0`. All are in the current
  `ISC,BSD-3-Clause,Zlib` organization allowlist? **No**: MIT and Apache-2.0
  are not listed in that configured allowlist. The maintainer must authorize
  the organization allowlist change through OA4 before adoption; this lane
  changed no organization setting.
- These crates are not yet adopted in this workspace, so this measurement adds
  no stale DEP-001 exception or vet record. C10/C19/C22b must run the actual
  integrated `cargo vet --locked` and add precise DEP-001/vet records at
  adoption, then remove each exception only when its measured duplicate is
  absent. No safe-to-deploy or local vet verdict is claimed here.

## Reproduction commands

From the workspace root; scratch manifest is outside the repository:

```sh
PATH="$HOME/.cache/maestro/tools/bin:$PATH" CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 ~/.local/bin/capped cargo tree --manifest-path /tmp/c09-measure/Cargo.toml -e features
PATH="$HOME/.cache/maestro/tools/bin:$PATH" CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 ~/.local/bin/capped cargo tree --manifest-path /tmp/c09-measure/Cargo.toml -d
PATH="$HOME/.cache/maestro/tools/bin:$PATH" CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 ~/.local/bin/capped cargo tree --workspace -d --locked
PATH="$HOME/.cache/maestro/tools/bin:$PATH" CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 ~/.local/bin/capped cargo metadata --manifest-path /tmp/c09-measure/Cargo.toml --format-version 1
PATH="$HOME/.cache/maestro/tools/bin:$PATH" CARGO_BUILD_JOBS=3 CARGO_INCREMENTAL=0 ~/.local/bin/capped cargo metadata --format-version 1
```

Raw scratch evidence is temporary under `/tmp/c09-measure`; the reproducible
manifest and command lines are retained here, not generated build outputs.
