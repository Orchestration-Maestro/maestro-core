//! `maestro`, the command line of Maestro (plan D12, FR-S1-012): noun, then
//! verb, as its users and their agents type it.
//!
//! - `maestro knowledge collection add <collection.json>` adds a collection
//!   from its strict declaration, or takes its new version;
//! - `maestro knowledge import --collection <id>` imports the collection's
//!   corpus manifests as a job, its ID printed first;
//! - `maestro knowledge quality --collection <id>` gives each revision of the
//!   collection its quality disposition, as a job, its ID printed first;
//! - `maestro knowledge status --collection <id>` reports its documents, its
//!   revisions by status and by disposition, and its generations;
//! - `maestro job wait <id>` follows a job until it ends, and exits with its
//!   outcome;
//! - `maestro setup` previews the search service Maestro needs, and installs
//!   it with `--yes`;
//! - `maestro status` summarizes which services and collections are ready;
//! - `maestro doctor` checks the kernel, the search service, the model
//!   router and each role's model card, naming the next action for every
//!   failure.
//!
//! # Output and exit codes
//!
//! A command prints text for people. Under `--json`, anywhere on the line,
//! it prints one JSON document on stdout instead, whose first member,
//! `schema`, names the document and its version, such as
//! `maestro-cli/import/1`. The members of an object the kernel keeps as JSON,
//! an outcome or a count by name, come in the order of their names.
//! Diagnostics go to stderr only. A command exits with 0 when it is done, 1
//! when the operation failed, a job that failed or was cancelled included,
//! and 2 for a usage error or a refused input: an unknown collection or job,
//! a declaration, binding, manifest or `config.toml` that is not what it must
//! be, or a resource another job holds under a live lease. A long command
//! prints its job's ID before anything else, as `job <id>`: stdout's first
//! line, or under `--json` stderr's, and the document's first member after
//! `schema`.
//!
//! Every command opens the kernel of the directories the environment names
//! (`maestro_kernel::paths`), applies `config.toml` to the local principal,
//! and reads through what that principal may read: a collection or a job
//! outside its grants is unknown. `setup` opens no kernel, and `status` and
//! `doctor` open it only when its database exists and lacks no migration
//! this build carries, never creating or migrating it. In the data directory
//! a command touches only `kernel.sqlite3`, `artifacts/` and, for `setup`,
//! `qdrant/`, never the files maestro v1 left there.
//!
//! # `knowledge collection add`
//!
//! Parses the declaration strictly, `maestro-collection/1`, and refuses one
//! whose collection the local principal cannot read. It records the
//! collection and its sources as an import does, keeps the declaration's
//! bytes as a pinned artifact, and journals its addition as
//! `maestro.knowledge.collection.added.v1` on the collection's stream,
//! `collection/<id>`, with the collection, the declaration's digest and the
//! file's absolute path, whose directory the declaration's quality ledger and
//! evaluation suite are relative to. That event is internal: it never leaves
//! the kernel's journal on this machine. The other commands find a
//! collection's declaration in the addition journaled last. Adding the same
//! declaration from the same file again changes nothing.
//!
//! ```json
//! {"schema":"maestro-cli/collection-add/1","collection":"synthetic",
//!  "declaration":"<sha256>","path":"/…/collection.json","sources":["handbook"],
//!  "changed":true}
//! ```
//!
//! # `knowledge import`
//!
//! Runs the import of the collection's declaration last added as a job of the
//! kind `knowledge.import`, in the collection's scope, holding the resource
//! `collection/<id>/import`. Its frozen inputs, which give its idempotency
//! key with its kind and scope, are
//! `{"collection": <id>, "declaration": <sha256>, "manifests": {<source>: <sha256>}}`,
//! the digest of each source's manifest read through `bindings.toml`. The
//! command holds the job's lease, `maestro-cli/<process ID>`, for 60 s at a
//! time: a heartbeat thread renews it every 20 s, and each step renews it
//! too. A step is journaled after every hundredth line of each manifest and
//! after its last, with the report's counts so far:
//! `{"imported", "unchanged", "held", "refused"}`. The job ends succeeded with
//! the import's report, refused entries included, or failed with
//! `{"error": <why>}`; a lease another process took over ends the command
//! with 1, the job left to that process.
//!
//! The same command again, with the same declaration and manifests, finds
//! the job of its key. One that succeeded is printed as it ended. One another
//! process holds is followed, each event printed for people as `job wait`
//! prints it, and its lease is tried again about once a second: once that
//! lease expired, the command takes the job over, as it does at once with a
//! lease that expired before it started, and imports again, which records
//! only what is missing.
//!
//! Another job on the collection's import, an import of other inputs, is
//! superseded when no live lease holds it, because its lease expired or no
//! process took it: the command takes its lease, cancels it with
//! `{"superseded_by": {"inputs": <this import's inputs>}}`, then submits its
//! own job again, once. One a live lease holds refuses the import, naming
//! its job.
//!
//! ```json
//! {"schema":"maestro-cli/import/1","job":"<ulid>","kind":"knowledge.import",
//!  "attempt":1,"state":"succeeded","outcome":{"collection":"synthetic",
//!  "held":0,"imported":28,"refusals":[],"refused":0,"unchanged":0}}
//! ```
//!
//! # `knowledge quality`
//!
//! Runs the quality gate (`maestro_knowledge::quality`) over the collection
//! as a job of the kind `knowledge.quality`, in the collection's scope,
//! holding the resource `collection/<id>/quality`, as the import runs its
//! own: its ID first, a heartbeat renewing its lease while the gate runs, a
//! rerun finding the job of its key, and another job on the resource
//! superseded or refusing it. Its frozen inputs are
//! `{"collection": <id>, "declaration": <sha256>, "ledger": <sha256 or null>,
//! "revisions": <sha256>}`: the digest of the quality ledger the declaration
//! names, relative to the file it was added from, null when there is no such
//! file; and that of the revisions the local principal reads, their IDs in
//! record order, each followed by a line feed. So the same command again
//! decides nothing twice, and after an import that recorded new revisions it
//! is a new job, which decides them. A ledger that is not strict is refused
//! before any job. The job ends succeeded with the gate's report, or failed
//! with `{"error": <why>}`; what the gate decided before it stopped stays
//! decided:
//!
//! ```json
//! {"schema":"maestro-cli/knowledge-quality/1","job":"<ulid>",
//!  "kind":"knowledge.quality","attempt":1,"state":"succeeded",
//!  "outcome":{"collection":"synthetic","decided":28,"held":[],"kept":0,
//!   "outcomes":{"accepted":28,"accepted_with_warnings":0,"excluded":0,
//!    "needs_reextraction":0,"quarantined":0},"revisions":28,"rules":{}}}
//! ```
//!
//! # `knowledge status`
//!
//! Its document is `maestro-cli/knowledge-status/1`, beside the top-level
//! `status`'s own; the revisions by status and the dispositions come in the
//! order of their names:
//!
//! ```json
//! {"schema":"maestro-cli/knowledge-status/1","collection":"synthetic",
//!  "title":"…","documents":28,
//!  "revisions":{"failed":0,"valid":…,"valid_with_warnings":…},
//!  "dispositions":{"accepted":0,"accepted_with_warnings":0,"excluded":0,
//!   "needs_reextraction":0,"quarantined":0,"undecided":28},
//!  "generations":[{"id":1,"state":"published","chunk_set":"…",
//!   "embedding_profile":"…","sparse_profile":"bm25-en-fr/1","point_count":3,
//!   "published_at":"…"}]}
//! ```
//!
//! # `setup`
//!
//! Installs Qdrant 1.19.1, the search service, as the systemd user unit
//! `maestro-qdrant.service`: its archive is downloaded over HTTPS with the
//! system's `curl`, refused unless its SHA-256 and that of the binary it
//! holds are the ones pinned in the code, and the binary goes under the data
//! directory, `qdrant/bin/qdrant`, with the service's storage and snapshots
//! beside it. The service binds 127.0.0.1 only, HTTP on 6333 and gRPC on
//! 6334, with telemetry off. Without `--yes` the command lists the steps the
//! machine lacks and changes nothing; with it, it takes them. A machine with
//! everything in place has no step, and a run then changes nothing. It
//! installs on Linux on x86-64 with systemd only; elsewhere it prints the
//! manual steps and exits 2.
//!
//! ```json
//! {"schema":"maestro-cli/setup/1","version":"1.19.1","service":"maestro-qdrant.service",
//!  "binary":"/…/qdrant/bin/qdrant","storage":"/…/qdrant/storage",
//!  "snapshots":"/…/qdrant/snapshots","unit":"/…/systemd/user/maestro-qdrant.service",
//!  "http":"127.0.0.1:6333","grpc":"127.0.0.1:6334",
//!  "steps":["install","write_unit","reload","enable","restart"],"changed":false}
//! ```
//!
//! # `status`
//!
//! Reports the kernel, Qdrant and the model router, each ready or not with
//! what its check saw, and each collection the local principal reads with
//! its documents and its published generation. It exits 0 whatever is down.
//!
//! ```json
//! {"schema":"maestro-cli/status/1",
//!  "services":[{"name":"kernel","target":"/…/kernel.sqlite3","ready":true,"detail":"intact"},
//!   {"name":"qdrant","target":"http://127.0.0.1:6333","ready":true,
//!    "detail":"Qdrant 1.19.1 answers"},
//!   {"name":"router","target":"http://127.0.0.1:8080/","ready":true,
//!    "detail":"16 entries in its catalog"}],
//!  "collections":[{"collection":"ctm","title":"…","documents":7131,
//!   "published":{"generation":3,"points":51234}}]}
//! ```
//!
//! # `doctor`
//!
//! Runs every check, in this order: `config.toml` and `bindings.toml`; the
//! kernel's database, which must exist, lack no migration this build carries
//! (a missing one is named, never applied), open, pass SQLite's quick check
//! and take `config.toml`'s grants; the artifact tree, each recorded artifact
//! present and intact; Qdrant answering as the pinned version; the model
//! router, at `MAESTRO_ROUTER_URL` or else `http://127.0.0.1:8080`, listing
//! its catalog, which starts no model; and each role's model card. A failed
//! check names its next action. It then lists what it found but must not
//! touch: the entries of the data directory the kernel does not own, as the
//! files maestro v1 left there, and the grants of `config.toml` that reach
//! no scope the kernel knows. It deletes nothing, and exits 0 when every
//! check passed and 1 when one failed.
//!
//! ```json
//! {"schema":"maestro-cli/doctor/1",
//!  "checks":[{"name":"database","target":"/…/kernel.sqlite3","passed":true,
//!    "detail":"intact","next_action":null},
//!   {"name":"model_card","target":"embedder","passed":false,
//!    "detail":"no model card is recorded for the embedder","next_action":"…"}],
//!  "untouched":["/…/ledger.sqlite3","/…/material"],
//!  "unreached_grants":["workspace/other"]}
//! ```
//!
//! # `job wait`
//!
//! Follows the job's stream, each event printed for people as
//! `<sequence> <type> <data>`, until the job ends, then prints it as it
//! ended, with the fields of the import's document:
//!
//! ```json
//! {"schema":"maestro-cli/job-wait/1","job":"<ulid>","kind":"knowledge.import",
//!  "attempt":1,"state":"failed","outcome":{"error":"…"}}
//! ```

mod cli;

use std::process::ExitCode;

/// Runs the command the arguments name, and exits with its code.
fn main() -> ExitCode {
    cli::main()
}
