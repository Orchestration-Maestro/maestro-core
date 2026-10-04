# 07 Extensibility: entry points, exit points and extensions

Owner requirement (2026-09-24): integrations **plug in and plug out**, consume
events to trigger other work, and never require a change to the core
application; the design must scale without a fixed limit.

The design gives Maestro two stable, versioned contracts and one supervised way
to run third-party code:

| Contract | Direction | What crosses it |
| --- | --- | --- |
| **Operations API** | Entry point (into Maestro) | Typed, versioned **commands** (`knowledge.publish`, `run.start`, …) under an authenticated principal |
| **Event stream** | Exit point (out of Maestro) | Typed, versioned **events** (`maestro.run.completed.v1`, …) read from the kernel journal with durable cursors |
| **Extension host** | Both | Supervised, sandboxed extension processes that subscribe to events and call operations, each with its own least-privilege principal |

The core never knows a specific integration. Adding, removing or replacing an
integration is a catalog change plus an activation, not a core release.

```mermaid
flowchart LR
  subgraph entry[Entry points]
    cli[CLI] & mcp[MCP] & http[Local HTTP API] & sched[Schedules] & watch[Source watchers] & hookin[Inbound webhooks]
  end
  entry -->|commands| admit{Admission<br/>principal + Cedar}
  admit --> ops[Application operations]
  ops --> journal[(Kernel journal<br/>ordered, append-only)]
  journal -->|cursors| subs[Subscriptions]
  subs --> builtin[Built-in consumers (S2)<br/>projections, telemetry]
  subs --> ext[Extension host<br/>sandboxed processes]
  subs --> hookout[Outbound webhooks]
  subs --> bridge[Broker bridge<br/>NATS JetStream, Kafka]
  ext -->|MCP tool calls| admit
```

## 1. Principles

1. **One way in.** Every entry point turns a request into a command and goes
   through the same admission (identity, scope, Cedar) and the same application
   operations. No entry point owns logic.
2. **One way out.** Everything that happens is an event in the kernel journal.
   When wired, built-in projections (Qdrant, Neo4j), telemetry and third-party
   integrations consume the same stream the same way.
3. **Out of process first.** Third-party code never runs inside the core
   process. A crashing, slow or hostile extension cannot corrupt the kernel or
   block the core.
4. **Declared, reviewed, activated.** An extension is catalog content: declared
   with its subscriptions, commands, effects and resources, reviewed by its
   owners and security, released in a bundle, then **activated** separately on
   a machine. Publication never grants execution.
5. **At-least-once, idempotent, ordered per subject.** Delivery survives
   restarts; consumers deduplicate by event ID; order holds per stream key.
6. **The journal is the outbox.** The core commits an event once; wired
   consumers receive it afterwards, from the journal. A failing consumer never
   rolls back or delays a committed operation.

## 2. Entry points

| Entry point | Transport | Principal | Slice |
| --- | --- | --- | --- |
| CLI | Process arguments, JSON output | Local user | S1 |
| MCP server | stdio (Streamable HTTP later, authenticated) | Host session, agent node | S1 |
| Local HTTP API | axum on a Unix socket or loopback with a token; `POST /v1/commands/{name}`, `GET /v1/jobs/{id}`, `GET /v1/events` (server-sent events) | Authenticated local client | S4 |
| Schedules | Daemon timers declared in configuration (`every`, `cron`) | The schedule's service principal | S4 |
| Source watchers | Per-source synchronization policy (one-off, manual, watch) | The source's service principal | S6 |
| Inbound webhooks | HTTPS receiver with signature verification (e.g. GitHub `X-Hub-Signature-256`), mapped to a declared command | A service principal per webhook | Later, on demand |
| Extensions | MCP tool calls through the same admitted operations; separate durable event protocol | `Extension::"<id>"` | S4 |

**Command contract.** A command has a stable name and major version
(`run.start/1`), a JSON Schema input, an **idempotency key** bound to the
command and its frozen inputs (a retried request returns the existing job), and
a typed result or job handle. Acknowledgement is not completion: long commands
return a job ID that `jobs.wait` follows. Command schemas are generated from the
Rust types (schemars) and published in the Operations API reference.

## 3. Exit points: the event stream

### 3.1 Event envelope

Events use the **CloudEvents 1.0** envelope, so any standard tooling can read
them:

```json
{
  "specversion": "1.0",
  "id": "01J9Z0Q7…",
  "source": "maestro://workstation-7/kernel",
  "type": "maestro.knowledge.generation.published.v1",
  "subject": "collection/ctm",
  "time": "2026-09-24T14:03:11.402Z",
  "datacontenttype": "application/json",
  "dataschema": "maestro://schemas/events/knowledge.generation.published/1",
  "maestroscope": "workspace/default/collection/ctm",
  "maestrostream": "collection/ctm",
  "maestrosequence": 18231,
  "traceparent": "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0bc902b7-01",
  "data": { "collection": "ctm", "generation": 8, "point_count": 81234 }
}
```

`maestrosequence` is the per-stream position; `traceparent` carries the W3C
trace context so a consumer's work joins the originating trace.
`maestrostream` names the stream that `maestrosequence` counts in,
`collection/<id>` for the knowledge family, so a consumer orders and
acknowledges events per stream, since sequences restart in each stream.

### 3.2 Event catalogue

| Family | Examples (all `.v1`) | Stream key |
| --- | --- | --- |
| Knowledge | `knowledge.import.completed`, `knowledge.revision.held`, `knowledge.generation.published`, `knowledge.generation.retired` | collection |
| Acquisition (S6) | `acquisition.run.completed`, `acquisition.item.withdrawn`, `acquisition.session.expired` | source |
| Catalog | `catalog.bundle.installed`, `catalog.bundle.revoked`, `catalog.extension.activated` | bundle |
| Runs | `run.started`, `run.interrupt.raised`, `run.node.completed`, `run.completed`, `run.cancelled` | run |
| Policy | `policy.decided` (denials and approvals only, redacted arguments) | run |
| Memory (S7) | `memory.checkpoint.committed`, `memory.restore.delivered` | session |
| Analysis (S8) | `analysis.job.ready`, `analysis.finding.submitted`, `analysis.contract.recorded`, `analysis.specification.promoted` | target, contract, specification |
| Provenance (S8) | `provenance.component.registered`, `provenance.obligation.raised` | component |
| Operations | `job.failed`, `health.degraded`, `backup.completed` | job, component |

Only events marked **public** in the catalogue reach extensions; internal events
(for example raw tool arguments) stay inside the core. Each public event type has
a JSON Schema generated from its Rust type.

**Versioning.** Within a major version, changes are additive only (new optional
fields). A breaking change introduces `.v2`; during a declared deprecation
window the core emits both. A CI compatibility test compares every schema with
its released predecessor.

### 3.3 Subscriptions and delivery

| Aspect | Design |
| --- | --- |
| Cursor | One durable cursor per subscription in the kernel (the B2 journal's `cursor`/`ack`) |
| Filter | Event-type patterns (`maestro.run.*`), scopes, subjects; scope filtering is enforced by the core against the subscriber's grants, not trusted from the filter |
| Start | From now, or replay from a position within retention (a new projection rebuild uses the same path) |
| Delivery | At least once; ordered per stream key; a subscriber acknowledges each event (or a batch) after processing |
| Backpressure | Bounded in-flight events per subscriber; the subscriber's lag is a metric and a health signal |
| Retries | Exponential backoff with jitter; after N failures the event moves to the subscriber's **dead-letter** list, inspectable and replayable with `maestro extension replay` |
| Isolation | Each subscriber has its own cursor; a stuck subscriber never delays another or the core |
| Redaction | Payload fields carry a classification; a subscriber receives only the classes its grant allows |

## 4. Extensions

### 4.1 What an extension can be

| Integration role | Does | Example |
| --- | --- | --- |
| `subscriber` | Reacts to events | Post a summary to a chat channel when `run.completed`; open an issue when `knowledge.revision.held` |
| `notifier` | Outbound webhook with a signed payload | Notify a team service on `catalog.bundle.revoked` |
| `bridge` | Relays the stream to a broker | NATS JetStream or Kafka for team-scale consumers |
| `source-connector` | Leases frontier items and submits captures through `knowledge.capture.submit` | Private vendor connectors (ADR-0009): the frontier stays core-owned |
| `extractor` | Converts one media type to canonical Markdown under the extractor contract ([01 §3](01-knowledge-pipeline.md#3-l2-extraction-and-normalization)) | A format the core does not support |
| `tool-provider` | Exposes agent tools | A checked server record in `core/backends/mcp/config.toml` or a selected package's registered add-or-narrow binding; require the binding owner's package and, for extension tools, its qualified extension ID ([03](03-agent-orchestration.md)) |
| `exporter` | Writes projections or reports elsewhere | A dashboard feed, an archive |
| `analyzer` | Leases an analysis job and submits findings about one target revision, with their evidence, coverage and limits ([09 §8](09-reverse-engineering.md#8-analyzers-are-extensions)) | A licence scanner, a code-property-graph query runner, an authorized traffic recorder |

These are integration roles, not descriptor `kind` values. The v4 descriptor
uses `tools` or `hook-subscriber`; a raw MCP server record has no resource ID.
Every action, including an event-triggered action, is an MCP tool call. S4
qualifies sandboxing and policy enforcement; S3 declarations do not prove it.
The separate extension event protocol supplies durable delivery, cursors and
acknowledgments, which MCP does not provide.

### 4.2 Declaration

Author v4 extensions with the pinned C58 schema and fixtures under
[S3 D14](../../specs/003-catalog/plan.md#d14-backends-preferences-and-extension-boundaries).
S3 Phase 2/X1 registers the descriptor and verified install/projection consumer
before S4 execution; an earlier checker reports the kind as unsupported.
The following field-level example replaces the old executable-command format;
it is not an approved connector or a claim of delivered runtime support.

The example root is `capabilities/operations/notifications/package.toml`.
Its `requires` includes `extension:notifications/run-notifier`; the resource is
`extensions/run-notifier/extension.toml` below that root. Its common metadata
records the registered schema, `package:notifications`, maturity, architecture
rows and exact qualified `requires`. Owners/maintainers derive from the owning
`package.toml`, never a repeated resource owner field. Products are values,
not source paths or IDs.

| Descriptor field or contract | Notification example |
| --- | --- |
| `name`, `version`, `description` | `run-notifier`, `1.0.0`, a description of the run-summary tool |
| `type`, `kind` | Synthetic integration value `notification-service`; `kind = "tools"` |
| Runtime and entry | Declared runtime name/version requirements; relative entry `code/run-notifier` within verified code, resolved through local runtime authority |
| Code source | Explicit local inventory containing that entry, covered by the signed package digest; alternatively one immutable version/digest/platform-assets/expected-signer release, never both |
| MCP and tools | Declared MCP protocol requirements; tool `notify` references `contract:notifications/notify-input` and `contract:notifications/notify-output` |
| Configuration | Config-schema reference `contract:notifications/notifier-config`; all three JSON contracts and their `.maestro.toml` sidecars are inventoried and required |
| Secrets and egress | Typed secret references only, for example `{ env = "NOTIFICATION_API_TOKEN" }`; declared ceiling `https://chat.example.org/api/`, not a runtime grant |
| Tests and evals | Explicit test/eval references and inventoried synthetic inputs, expected outputs and refusals; missing, stale or zero cases fail |
| Hook subscribers | For `kind = "hook-subscriber"`, also name supported D13 hook points; their event schemas are implicit platform protocols, not contract resources |

`requires` contains only qualified catalog IDs, including referenced contracts
and standard-owned policy resources, never MCP server or protocol names. The
checker rejects missing contracts, mutable code, escaping entries, shell command
strings, install/build scripts and literal secrets. No check or install resolves
secrets, installs a runtime or launches code. Each extension needs a separate
trusted Cedar grant bound to its principal, package/code digest and destinations;
its declared egress ceiling cannot grant access.

**Durable events remain separate.** For this tool example, S4 subscribes to
`maestro.run.completed.v1` / `maestro.run.cancelled.v1` in admitted project scopes,
then invokes `notify` over MCP. Those run events are not extra hook points.
Delivery retains the journal cursor/acknowledgment protocol in §3; MCP supplies
only the action. Runtime compatibility retains `maestro-events` ^1 and
`maestro-operations` ^1 outside catalog `requires`. S4 checks known public event
majors, admitted operations, scopes/data classes, Cedar effects and resource
ceilings before activation; S3's static descriptor check cannot qualify them.

### 4.3 The extension host

The daemon ([03 §2.4](03-agent-orchestration.md#24-the-engine-durable-event-sourced-execution))
supervises extensions:

- **Process isolation**: each extension runs in the same sandbox as `step`
  nodes (Landlock, seccomp, network namespace with its egress allowlist,
  cgroup limits). Process separation alone is not treated as a security
  boundary.
- **Protocols**: the versioned *Maestro Extension Protocol* uses JSON-RPC 2.0
  over the process's stdio for `initialize` (versions, capabilities),
  `events.deliver` / `events.ack` (push)
  or `events.poll` (pull), `health` and `shutdown` for durable delivery and
  lifecycle. Actions are MCP tool calls, including admitted operation calls;
  MCP never substitutes for event acknowledgments. Any language can implement
  these out-of-process contracts.
- **Principal**: `Extension::"extension:notifications/run-notifier"` in Cedar with exactly the declared
  grants; an extension cannot widen its own grants, and its outputs are
  untrusted data like any tool output.
- **Lifecycle**: start on activation, health checks, restart with backoff,
  graceful stop on deactivation; the runtime's **stop-all** stops every managed
  extension and disables schedules and watchers, and a supervisor never undoes
  it.
- **Hot plug**: activation, deactivation and upgrade happen without restarting
  the daemon or rebuilding the core.

```text
maestro extension list                      # declared in installed bundles, with state
maestro extension inspect extension:notifications/run-notifier
maestro extension activate extension:notifications/run-notifier     # explicit, audited
maestro extension deactivate extension:notifications/run-notifier   # cursor kept
maestro extension replay extension:notifications/run-notifier --dead-letter
maestro extension remove extension:notifications/run-notifier       # confirm cursor deletion
```

### 4.4 Later options

- **WebAssembly components** (wasmtime + WIT interfaces) for small, pure,
  latency-sensitive extensions (filters, scorers, extractors): stronger
  in-process isolation than native plugins. Added only when an extension needs
  in-process speed.
- **Native dynamic libraries are excluded**: an unstable ABI and shared memory
  would let an extension crash or corrupt the core.

## 5. Scaling

| Level | Mechanism |
| --- | --- |
| Laptop | The SQLite journal and in-process delivery; hundreds of events per second are far above a single user's needs |
| Many consumers | Independent cursors; each subscriber scales on its own; the core's cost per extra subscriber is one cursor |
| Heavy consumers | A subscriber can shard its work by stream key; ordering holds per key |
| Team or cloud | A `bridge` extension relays public events to NATS JetStream (or Kafka), where any number of services subscribe with their own retention; the core is unchanged and remains the only authority |

The core never embeds a message broker; a broker is an exit point like any other.

## 6. Security

- Events are **data**; commands from extensions go through the same admission
  and Cedar evaluation as any caller.
- Outbound webhooks are signed (HMAC-SHA256 over the body and a timestamp) and
  restricted to declared destinations by the egress policy.
- Inbound webhooks verify the sender's signature before any command is created;
  an unverified request is dropped and counted.
- Activation, deactivation, grant changes and replays are journaled with the
  actor.
- A revoked bundle (see [03 §1.3](03-agent-orchestration.md#13-check-compile-release-install))
  deactivates its extensions at the next revocation check.

## 7. Delivery by slice

| Slice | Delivers |
| --- | --- |
| S1 | Journal with per-stream sequences, durable cursors and acknowledgements; the event catalogue starts with knowledge events |
| S2 | Built-in projection and telemetry consumers use the tested S1 cursor primitive |
| S3 Phase 2/X1 | V4 extension descriptors, contracts/fixtures, artifact verification and install/projection; zero launch or execution qualification |
| S4 | Extension host in the daemon, Maestro Extension Protocol, process extensions, outbound webhooks, local HTTP API with server-sent events, schedules; a reference `echo` extension in CI |
| S5 | Capabilities use subscribers (for example notifications); a contributed extension ships through the catalog |
| S6 | Source connectors as extensions; the private vendor connectors run this way |
| S8 | Analyzer tools and their contract on the v4 extension descriptor; analysis and provenance events; analyzers run under an `analysis/<target>` scope and reach nothing else ([09](09-reverse-engineering.md)) |
| Later | Inbound webhooks, broker bridge, WebAssembly components, on demand |

## 8. Tests

| Test | Proves |
| --- | --- |
| Schema compatibility | No breaking change inside a major version |
| Redelivery | Killing the daemon or the extension mid-delivery loses nothing and duplicates are recognized by event ID |
| Slow and failing consumers | The core and other subscribers are unaffected; the dead-letter path works |
| Scope and redaction | A subscriber never receives events or fields outside its grants |
| Sandbox | An extension cannot reach undeclared hosts or paths |
| Hot plug | Activate, upgrade and deactivate without restarting the daemon |
| Stop-all | Every extension stops and stays stopped until an authorized start |
