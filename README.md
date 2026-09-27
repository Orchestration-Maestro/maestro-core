# maestro-core

[![CI](https://github.com/Orchestration-Maestro/maestro-core/actions/workflows/ci.yml/badge.svg)](https://github.com/Orchestration-Maestro/maestro-core/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/Orchestration-Maestro/maestro-core/badge)](https://scorecard.dev/viewer/?uri=github.com/Orchestration-Maestro/maestro-core)

The local runtime of Maestro: a knowledge kernel, retrieval and a command-line
application. The workspace contains `maestro-canonicalization`,
`maestro-conventions`, `maestro-kernel`, `maestro-knowledge` and `maestro`.
[`maestro-canonicalization`](crates/maestro-canonicalization/README.md) turns
Markdown into provenance-bearing canonical documents, groups duplicates and
cuts them into token-budgeted chunks; later capabilities arrive slice by slice
([roadmap](docs/architecture/06-roadmap.md)).

## Start here

| To | Read |
| --- | --- |
| Understand the design | [docs/architecture](docs/architecture/README.md) |
| See why a choice was made | [docs/adr](docs/adr/README.md) |
| Learn the vocabulary | [CONTEXT.md](CONTEXT.md) |
| Follow the active slice | [S1 knowledge kernel](specs/001-knowledge-kernel/spec.md) |
| Work in this repository | [AGENTS.md](AGENTS.md) |

## Develop

The crates build and pass their tests on Linux, macOS and Windows; CI runs them
on all three ([ADR-0018](docs/adr/0018-rustix-on-unix-and-win32-flags-on-windows.md)).
Install [rustup](https://rustup.rs), then the organization's gate from the
[latest rust-workflows release](https://github.com/Orchestration-Maestro/rust-workflows/releases/latest),
which installs the toolbelt CI runs, at the versions it runs, and the commit
hooks ([details](https://github.com/Orchestration-Maestro/rust-workflows/blob/main/docs/ci.md#the-tools-on-your-machine)).
`just check` runs the full local CI check over the commits a push sends. In the
normal workflow, the pre-push hook runs it
([details](https://github.com/Orchestration-Maestro/rust-workflows/blob/main/docs/ci.md#run-ci-before-you-push)). Active S1 work follows the temporary process in
[tasks.md](specs/001-knowledge-kernel/tasks.md#current-s1-integration-workflow):
task branches use targeted checks and one reviewed integration pull request;
those lane checks do not replace full CI.

```bash
cargo install --locked --git https://github.com/Orchestration-Maestro/rust-workflows \
  --tag vX.Y.Z rust-gate   # the latest release's tag
rust-gate setup   # the pinned toolbelt, and the commit hooks
just check        # full local CI check for the normal repository workflow
just native       # the native tokenizer tests, through a local binding
```

The native tests read the tokenizer's artifacts where a binding file says they
are: see [TOKENIZER.md](crates/maestro-canonicalization/TOKENIZER.md).

## Licence

MIT: [LICENSE](LICENSE).
