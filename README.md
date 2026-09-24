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

On Linux x86_64 with rustup:

```bash
scripts/bootstrap.sh   # the pinned toolbelt in .tools/, and the commit hooks
just check             # the local gate; it must pass before every push
just native            # the native tokenizer tests, through a local binding
```

The native tests read the tokenizer's artifacts where a binding file says they
are: see [TOKENIZER.md](crates/maestro-canonicalization/TOKENIZER.md).

## Licence

MIT: [LICENSE](LICENSE), [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).
