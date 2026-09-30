# Prepare and check the local knowledge graph

The knowledge graph is an optional projection of the kernel into LadybugDB, an
embedded engine that runs inside the `maestro` process. It needs no graph
server, port, Docker or JVM, and nothing is downloaded on first use. The kernel's
SQLite database and its artifacts stay the only authority: every graph file can
be rebuilt from them.

The graph is off by default (`graph.engine = "none"`). Then `setup`, `status`
and `doctor` never look for graph files.

## Build graph claims

`maestro knowledge graph build` uses exactly one extractor. Rule builds keep
using `--rule PATH`. Offline model builds require all three explicit options:
`--extractor-card DIGEST`, `--window-policy PATH`, and `--token-budget N`.
The card must be a registered `Extractor` card. The versioned policy is closed
JSON, for example:

```json
{
  "schema": "maestro-graph-window-policy/1",
  "max_window_bytes": 4096,
  "overlap_bytes": 128,
  "max_windows": 64
}
```

Tune that policy only on pilot/synthetic development failures, never held-out
labels or results. Model replies provide typed candidates and a verbatim quote
pointer; Maestro accepts a quote only when it occurs exactly once in the same
source window and records the source's original revision, block, byte span and
digest as evidence. Source instructions cannot change the closed vocabulary or
build authority. The token budget is an estimate limit for one job: the card
tokenizer counts system-prompt-plus-window input and reserves the full
1,024-token output estimate per call. Resume reruns the whole-plan preflight,
but reuses committed batches. A crash can restart at most one source across
that job's lease-resume chain; the estimated spend is therefore at most the
plan estimate plus one largest-source window cost. Rerunning a failed job
starts a new job and spends again, up to its own estimate. This is not an exact
spend cap; per-window durable token accounting is deferred. The card, prompt,
policy, profile and budget digests remain frozen in job inputs. The policy
digest hashes the policy file bytes, so even whitespace changes its digest.

## Select the engine

Build `maestro` with the `engine` feature to unlock the `lbug` setting:

```sh
cargo build --release -p maestro --features engine
```

Today the feature only unlocks the setting; the engine arrives with G27.

Select it for every run, or for one run:

```sh
maestro config set graph.engine lbug
maestro --set graph.engine=lbug status
```

`graph.engine` accepts `none` or `lbug`. No setting or flag takes a graph file
path: Maestro places the graph under its own data directory. A build without the
engine refuses the graph part of `setup` when `lbug` is selected, but still
runs the search-service part; `status` and `doctor` report the engine as
missing from the build.

## Create the graph's directory

```sh
maestro --set graph.engine=lbug setup        # preview: changes nothing
maestro --set graph.engine=lbug setup --yes  # create it
```

Setup previews, and with `--yes` creates, `<data directory>/graph`, where the
data directory is `maestro` under `XDG_DATA_HOME` (or the platform's data home).
On Unix, it gets mode `0700`; setup also restores that mode on an existing
directory. Setup downloads nothing, opens no database and removes nothing. It
refuses a graph directory that is a link or not a directory.

The graph's part comes first and runs on every platform. The search service's
part follows unchanged. If the search-service part is refused, setup prints
only the graph's part first (`maestro-cli/setup-graph/1` under `--json`), then
reports the unchanged search-service refusal and exit code. If the graph part
is refused, setup reports its detail and still runs the search-service part;
the search-service failure's exit code takes precedence.

## Check it

`status` and `doctor` include a read-only `graph` check. It never creates,
repairs, migrates, fetches or deletes a file.

| State | What it says | Next action |
| --- | --- | --- |
| Off | the graph is off | none; `doctor` counts it as not checked |
| Engine missing | built without the engine | use a build with the `engine` feature, or set `graph.engine` to `none` |
| Directory missing | the graph directory is missing | `maestro setup --yes` |
| Permissions | its permissions let others in | `maestro setup --yes`, which restores mode `0700` |
| Relocated | the graph directory is a link | stop every `maestro`, move the target into place |
| Nothing published | no graph is published yet, so no file was opened | none; the first projection build publishes one |
| Locked | a writer holds the graph file's lock | let the writer finish, then run `doctor` again |
| Corrupt | the file does not open read-only, or does not answer | rebuild the projection; keep the files |

Graph files come only from the kernel's projection receipt. The check refuses
a listed file outside the graph directory, or reached through a link, before
anything opens it. It then opens each file read-only, runs `RETURN 1`, closes
it, and does both again. Locks differ by platform: on Windows a writer's lock
refuses every reader, while on Linux and macOS the lock is advisory and a reader
can open beside a writer.

**Today no graph is published.** The receipt and the engine adapter that opens
graph files arrive with G27, so every enabled check stops at "no graph is
published yet" and opens no file.

## Move the data directory

1. Stop every `maestro` process, readers included.
2. Move the whole data directory, `graph` included, to its new place.
3. Set `XDG_DATA_HOME` (or the platform's data home) to the new base for
   every `maestro` process.
4. Run `maestro doctor`.

Do not leave a link in place of `graph`: `doctor` reports it as relocated, and
setup refuses it.

## Rebuild or remove the graph

The graph is disposable. Never back up or restore a graph file as an authority,
and never copy one in by hand.

- **Rebuild offline.** The projection build, which G27 and G28 add, rebuilds
  the graph from the kernel's database and artifacts only, with no network.
- **Remove safely.** Stop every `maestro` process that may read or write the
  graph, and confirm none is left, because on Linux and macOS a lock does not
  keep a reader out. Then remove the files under `<data directory>/graph`
  only: never the kernel's database or artifacts beside it. The next
  projection build writes a new graph.

Maestro has no graph cleanup command yet. A command that refuses to remove
files while a reader holds them, and refuses files Maestro does not own, comes
with G27, the first task that writes graph files: before then there is no
receipt that says which files are the graph's.
