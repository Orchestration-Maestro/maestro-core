<p align="center">
  <img src=".github/assets/maestro-core.jpg" alt="Maestro Core: ask anything, cite everything." width="100%" />
</p>

<h1 align="center">🎼 Maestro Core</h1>

<p align="center">
  <strong>Ask anything. Cite everything.</strong><br />
  A local knowledge engine that imports documentation, searches cited passages and answers questions from your sources.
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Rust-2024-CE422B?style=for-the-badge&amp;logo=rust&amp;logoColor=white" alt="Rust 2024" />
  <img src="https://img.shields.io/badge/Design-local--first-334155?style=for-the-badge" alt="Local-first" />
  <img src="https://img.shields.io/badge/Agents-MCP-334155?style=for-the-badge" alt="MCP tools for agents" />
  <img src="https://img.shields.io/badge/Tests-Linux%20%7C%20macOS%20%7C%20Windows-334155?style=for-the-badge&amp;logo=githubactions&amp;logoColor=white" alt="Tests on Linux, macOS and Windows" />
</p>

<p align="center">
  <a href="https://github.com/Orchestration-Maestro/maestro-core/actions"><img src="https://img.shields.io/github/check-runs/Orchestration-Maestro/maestro-core/main?label=checks" alt="Checks on main" /></a>
  <a href="https://scorecard.dev/viewer/?uri=github.com/Orchestration-Maestro/maestro-core"><img src="https://api.scorecard.dev/projects/github.com/Orchestration-Maestro/maestro-core/badge" alt="OpenSSF Scorecard" /></a>
  <a href="https://codecov.io/gh/Orchestration-Maestro/maestro-core"><img src="https://codecov.io/gh/Orchestration-Maestro/maestro-core/graph/badge.svg" alt="Codecov line coverage" /></a>
</p>

## ⚡ Quick start

Install from source with [rustup](https://rustup.rs) installed:

```sh
cargo install --locked --git https://github.com/Orchestration-Maestro/maestro-core maestro
maestro setup --yes
maestro doctor
```

`setup --yes` installs digest-checked Qdrant 1.19.1 as a Linux x86-64 systemd
user service, bound to localhost with telemetry off. Run `maestro setup` to
preview its changes first; on other platforms it prints manual setup steps.
Start [maestro-model-router](https://github.com/Orchestration-Maestro/maestro-model-router)
with your local models at `http://127.0.0.1:8080`. `MAESTRO_ROUTER_URL` changes
that address; `MAESTRO_QDRANT_URL` selects an existing Qdrant gRPC endpoint.

On a fresh installation, `doctor` reports `no kernel database yet`. Adding a
collection creates it. Doctor checks the kernel and services, not the
qualification of each collection's model cards.

From a checkout of this repository, try the public French/English handbook in
[`tests/fixtures/synthetic`](tests/fixtures/synthetic). These Bash commands
configure a **fresh installation**; keep existing bindings and grants if you
already have collections:

```sh
export DEMO="$(mktemp -d)"
export XDG_CONFIG_HOME="${XDG_CONFIG_HOME:-$HOME/.config}"
export XDG_DATA_HOME="${XDG_DATA_HOME:-$HOME/.local/share}"
mkdir -p "$DEMO" "$XDG_CONFIG_HOME/maestro"
sed 's/"id": "synthetic"/"id": "readme-demo"/' \
  tests/fixtures/synthetic/collection.json > "$DEMO/collection.json"
printf 'synthetic_root = "%s/tests/fixtures/synthetic"\n' "$PWD" \
  > "$XDG_CONFIG_HOME/maestro/bindings.toml"
cat > "$XDG_CONFIG_HOME/maestro/config.toml" <<'TOML'
[access]
read = ["workspace/default/collection/readme-demo"]
TOML
maestro knowledge collection add "$DEMO/collection.json"
maestro knowledge import --collection readme-demo
maestro knowledge quality --collection readme-demo
```

Import and quality print a job ID, then their result. Shortened output:

```text
succeeded {"collection":"readme-demo","held":0,"imported":28,"refusals":[],"refused":0,"unchanged":0}
succeeded: 28 revisions, 28 decided, 0 kept as decided before
outcomes: accepted 28, accepted_with_warnings 0, excluded 0, needs_reextraction 0, quarantined 0
0 held
```

Model files and qualified cards are supplied separately: installation and
`setup` do not download models. Replace the quoted placeholders below with your
embedder's [model card](crates/maestro-kernel/src/gateway/card_v2/types.rs), its
pinned evidence directory and GGUF file:

```sh
maestro model register --collection readme-demo \
  --card "<embedder-card.json>" --evidence "<evidence-directory>" \
  --gguf "<embedder.gguf>"
maestro knowledge prepare --collection readme-demo --card "<embedder-card>"
maestro knowledge publish --collection readme-demo --card "<embedder-card>"
maestro knowledge search --collection readme-demo --query "How long are backups retained?"
maestro knowledge ask --collection readme-demo --question "How long are backups retained?"
```

`<embedder-card>` is the SHA-256 digest printed by registration, not a file
path. Register and select a qualified reranker, and register the answerer's
card, as described under [models](#-models). Prepare and publish print job
progress; search returns passages with source references and route outcomes;
ask returns a cited answer or a refusal. Both operations require a published
generation. The public [synthetic integration test](crates/maestro-knowledge/tests/it/synthetic_gate/policy_tests.rs)
exercises import, quality, preparation, publication, search and answers with
test models; those test cards are not real model downloads.

Working on this repository instead? Install [rustup](https://rustup.rs), then
`rust-gate setup` and `just check`; see [Develop](#-develop).

## 🎯 Objectives

- **Answers you can check:** claims cite stored passages; insufficient evidence
  produces a refusal, not an invented answer.
- **Private by default:** local models and a local search service.
- **One authority:** the kernel stores source evidence; search indexes are
  rebuilt from it.
- **Safe for agents:** MCP tools read only the collections the user grants.
- **Replaceable models:** model cards pin identities and qualification evidence,
  rather than binding callers to one provider.

## 🔄 How it works

<p align="center">
  <img src=".github/assets/how-it-works.svg" alt="A Markdown collection passes through import and quality, then chunking and embedding with the embedder's tokenizer. A verified generation publishes dense vectors and a French/English BM25 index together. Search fuses dense, lexical and identifier routes and reranks the best 30 candidates. Ask uses bounded evidence for a cited answer or a refusal, delivered through the CLI and MCP clients." width="100%" />
</p>

1. **Import and check quality.** A collection declaration binds corpus manifests
   to local Markdown. Import checks their bytes; quality assigns each revision
   a disposition. Only eligible revisions enter preparation.
2. **Chunk and embed.** The embedder's own tokenizer counts the prepared input,
   so structural chunks fit its budget. The model card pins the tokenizer,
   model file, runtime and input formats.
3. **Publish a generation.** Build dense vectors and French/English BM25 in a
   separate Qdrant collection. Verify its layout, count and every chunk's point
   before switching the published generation. A failed check leaves the old
   generation searchable.
4. **Search.** Dense, lexical and exact-identifier routes retrieve candidates.
   Reciprocal rank fusion combines them; the best 30 are reranked by default.
   Evidence carries the stored source reference, revision, span and generation,
   plus each route's outcome. Global inventory questions also use a scoped
   structured route.
5. **Ask.** The answerer reads at most 5 passages and 6,000 UTF-8 evidence bytes
   by default, answers in the question's language, and cites source spans.
   Unsupported output is checked, retried once, then refused. Empty evidence
   produces a refusal without calling the answer model.
6. **Use either surface.** The `maestro` CLI and four stdio MCP tools share the
   same scoped knowledge operations. A client never becomes an authority for
   collection access.

## 🧩 Commands

The command contracts below come from `maestro --help` and the subcommands'
`--help`. `--json` writes one result document to stdout and diagnostics to
stderr; `--set KEY=VALUE` overrides settings for one run.

| Command | Contract |
| --- | --- |
| `knowledge collection add` | Add a strict collection declaration, or accept its new version |
| `knowledge import` | Import corpus manifests as a leased job; print its ID first |
| `knowledge quality` | Assign a quality disposition to every revision in the collection |
| `knowledge prepare` | Prepare eligible revisions into a chunk set using a recorded embedder card |
| `knowledge publish` | Verify and publish a complete chunk set as a Qdrant generation |
| `knowledge verify` | Recheck the published generation as a job |
| `knowledge status` | Report the collection's documents, revisions, dispositions and generations |
| `knowledge collections` | List collection metadata visible to the local principal |
| `knowledge search` | Return bounded source-backed passages from the published generation |
| `knowledge get` | Retrieve an exact chunk or section from a visible published or retained generation |
| `knowledge ask` | Answer from verified passages, or refuse when they do not suffice |
| `model` | Register cards, qualify rerankers, select eligible cards and list registry state |
| `job wait` | Follow a job to completion and exit with its outcome |
| `eval` | Evaluate collection search and answers against a manifest-defined suite |
| `setup` | Preview the local search-service install; `--yes` takes its steps |
| `status` | Summarize service and collection readiness |
| `doctor` | Check kernel and service health; name the next action for each failure |
| `mcp` | Serve the four knowledge tools over stdio |
| `config` | Get, set, unset, list and explain settings; inspect their change history |
| `backup` | Copy the kernel database and recorded artifacts to a new or empty directory |
| `restore` | Validate a backup, then install it into a data directory without a kernel |

## 🤖 From your agent

| MCP tool | Contract |
| --- | --- |
| `knowledge_collections` | List the collections the local principal can read |
| `knowledge_search` | Return cited passages, generation identity, budgets and route outcomes |
| `knowledge_get` | Retrieve an exact chunk or section, optionally pinned to a generation |
| `knowledge_ask` | Return a source-backed answer or an explicit refusal |

Use the [MCP connection guide](docs/how-to/knowledge-mcp.md) to set
`MAESTRO_BIN` to the absolute executable path, `MAESTRO_CONFIG_HOME` and
`MAESTRO_DATA_HOME` to your installation's base directories, and
`MAESTRO_MCP_CONFIG` to Pi's private adapter configuration. The guide contains
that JSON file and the client permission steps. Bash registration forms:

### Claude Code

```sh
claude mcp add --transport stdio --scope user maestro --env "XDG_CONFIG_HOME=$MAESTRO_CONFIG_HOME" --env "XDG_DATA_HOME=$MAESTRO_DATA_HOME" -- "$MAESTRO_BIN" mcp
```

### Codex CLI

```sh
codex mcp add maestro --env "XDG_CONFIG_HOME=$MAESTRO_CONFIG_HOME" --env "XDG_DATA_HOME=$MAESTRO_DATA_HOME" -- "$MAESTRO_BIN" mcp
```

### Copilot CLI

```sh
copilot mcp add maestro --env "XDG_CONFIG_HOME=$MAESTRO_CONFIG_HOME" --env "XDG_DATA_HOME=$MAESTRO_DATA_HOME" --tools knowledge_collections,knowledge_search,knowledge_get,knowledge_ask -- "$MAESTRO_BIN" mcp
```

### Pi

With `pi-mcp-adapter`:

```sh
pi --mcp-config "$MAESTRO_MCP_CONFIG"
```

The server uses the local `local` principal. It reloads the user's grants
between calls and rechecks them before delivery; a client cannot grant itself
more access. Registering a server does not publish a collection or grant
permission. A remote agent provider is not made local by a local MCP process;
collection access and permission to send data to that provider are separate.

## 🧠 Models

The [model-card contract](crates/maestro-kernel/src/gateway/card_v2/types.rs)
records weights, tokenizer, formats, runtime, resource limits and qualification
evidence. Cards are registered per collection; models are not bundled.

| Local role | Selection contract |
| --- | --- |
| Embedder | The registered card passed to `prepare` and `publish`; no bundled default model |
| Reranker | The collection's selected, qualified reranker card; no bundled default model |
| Answerer | Gemma 4 E4B, non-thinking: the `ask.model` default is `ask-gemma4-e4b-nonthinking`, a router entry requiring a registered answerer card |

`maestro model register` imports a card and its pinned evidence. After a
reranker is registered, `maestro model check` records its real qualification;
`maestro model select` accepts only an eligible real evaluation of that exact
card and role. `maestro model list` shows the cards, evaluations and selections.
Embedder qualification runs through preparation. Model selection follows
[ADR-0011](docs/adr/0011-models-chosen-by-bake-off.md).

Serve replacement models through
[maestro-model-router](https://github.com/Orchestration-Maestro/maestro-model-router)
and register matching cards. The answerer's router entry is selected with
`ask.model` or `knowledge ask --model`, not `model select`.
`maestro config list` shows the effective settings and their source layer,
including rerank depth, evidence budgets and answer language.

## 🔒 Guarantees

Each row names the regression test that proves the boundary. These tests use
public synthetic evidence and test-owned services.

| Guarantee | Proof test |
| --- | --- |
| Only a complete, verified generation becomes searchable; a failed check keeps the previous one | [`the_alias_stays_while_a_generation_fails_its_check`](crates/maestro-knowledge/tests/it/qdrant_projection/alias_moves.rs) |
| An answer without evidence is refused without model generation | [`i5_empty_search_result_refuses_without_generation`](crates/maestro-knowledge/src/answer/tests.rs) |
| Unsupported commands in an answer are retried once, then refused | [`unsupported_command_is_retried_once_then_refused`](crates/maestro-knowledge/src/answer/tests.rs) |
| A failed route is reported while other routes retain their results | [`all_route_futures_start_together_and_one_failure_keeps_the_others`](crates/maestro-knowledge/src/search/tests/handoff.rs) |
| Expired routes and reranking report their deadline outcome | [`each_route_and_the_rerank_report_a_passed_deadline_as_its_code`](crates/maestro-knowledge/src/search/tests/stages.rs) |
| Revoked grants cannot return old evidence between MCP calls or through stale answer access | [`access_revoked_between_mcp_calls_does_not_return_the_old_chunk`](crates/maestro/tests/it/knowledge_get/mcp_and_authorization.rs), [`ask_refuses_a_stale_grant_reapplied_after_revocation`](crates/maestro/src/mcp/server/tests/stale_grants.rs) |
| Backup integrity is checked before the restore target is touched | [`restore_checks_file_integrity_before_touching_the_target_directory`](crates/maestro/tests/it/backup_restore.rs) |
| A lost search projection is rebuilt from stored kernel state as a new generation | [`again_rebuilds_a_missing_published_projection_as_a_new_generation`](crates/maestro-knowledge/tests/it/qdrant_projection/projection_rebuild/rebuild_tests.rs) |

## 💾 Backup and restore

Back up the authoritative database and artifacts, not the disposable search
index. The destination must be new or empty; restore refuses an existing
kernel. Using the scratch directory created above:

```sh
maestro backup --to "$DEMO/backup"
XDG_DATA_HOME="$DEMO/restored" maestro restore --from "$DEMO/backup"
```

Keep the installation's configuration and model files separately. A restore
does not recreate Qdrant automatically: republish the stored chunk set with
`knowledge publish --again --chunk-set` and its matching embedder card.

## 🔧 Develop

The crates build and pass their tests on Linux, macOS and Windows; CI runs them
on all three ([ADR-0018](docs/adr/0018-rustix-on-unix-and-win32-flags-on-windows.md)).
Install [rustup](https://rustup.rs), then the organization's gate from the
[latest maestro-rust-workflows release](https://github.com/Orchestration-Maestro/maestro-rust-workflows/releases/latest),
which installs the toolbelt CI runs, at the versions it runs, and the commit
hooks ([details](https://github.com/Orchestration-Maestro/maestro-rust-workflows/blob/main/docs/ci.md#the-tools-on-your-machine)).
`just check` runs exactly what CI runs, the steps of its checks job over the
commits a push sends, and the pre-push hook runs it
([details](https://github.com/Orchestration-Maestro/maestro-rust-workflows/blob/main/docs/ci.md#run-ci-before-you-push)):

```sh
rust-gate setup   # the pinned toolbelt and the commit hooks
just check        # CI's checks, here; it must pass before every push
```

## 📚 Documentation

- [Connect an agent over MCP](docs/how-to/knowledge-mcp.md)
- [Architecture](docs/architecture/README.md): the system, layers and invariants
- [Architecture decisions](docs/adr/README.md) and [domain glossary](CONTEXT.md)
- [Engineering](docs/standards/engineering.md), [security](docs/standards/security.md)
  and [Northstar](docs/standards/northstar.md): the organization's rules here
- [Banner credits](.github/assets/CREDITS.md): artwork, type and provenance

## Licence

MIT: [LICENSE](LICENSE).
