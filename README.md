# maestro-core

[![CI](https://github.com/Orchestration-Maestro/maestro-core/actions/workflows/ci.yml/badge.svg)](https://github.com/Orchestration-Maestro/maestro-core/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/Orchestration-Maestro/maestro-core/badge)](https://scorecard.dev/viewer/?uri=github.com/Orchestration-Maestro/maestro-core)

The local runtime of Maestro: knowledge kernel, retrieval, orchestration and
the command-line tools. Today it holds one crate,
[`maestro-canonicalization`](crates/maestro-canonicalization/README.md), which
turns Markdown into provenance-bearing canonical documents, groups duplicates
and cuts them into token-budgeted chunks. The rest arrives slice by slice
([roadmap](docs/architecture/06-roadmap.md)).

## Start here

| To | Read |
| --- | --- |
| Understand the design | [docs/architecture](docs/architecture/README.md) |
| See why a choice was made | [docs/adr](docs/adr/README.md) |
| Learn the vocabulary | [CONTEXT.md](CONTEXT.md) |
| Follow the active slice | [specs](specs/000-foundation/spec.md) |
| Work in this repository | [AGENTS.md](AGENTS.md) |

## Develop

The crates build and pass their tests on Linux, macOS and Windows; CI runs them
on all three ([ADR-0018](docs/adr/0018-rustix-on-unix-and-win32-flags-on-windows.md)).
Install [rustup](https://rustup.rs), then the organization's gate, which
installs the toolbelt CI runs, at the versions it runs, and the commit hooks
([details](https://github.com/Orchestration-Maestro/rust-workflows/blob/v4.0.0/docs/ci.md#the-tools-on-your-machine)).
The local gate runs on Linux:

```bash
cargo install --locked --git https://github.com/Orchestration-Maestro/rust-workflows \
  --tag v4.0.0 rust-gate
rust-gate setup   # the pinned toolbelt, and the commit hooks
just check        # the local gate; it must pass before every push
just native       # the native tokenizer tests, through a local binding
```

The native tests read the tokenizer's artifacts where a binding file says they
are: see [TOKENIZER.md](crates/maestro-canonicalization/TOKENIZER.md).

## Licence

MIT: [LICENSE](LICENSE).
