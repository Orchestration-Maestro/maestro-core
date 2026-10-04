# Extensions plug in through events and operations, out of process

Status: accepted, 2026-09-24.

Integrations must plug in and out without changing the core. Maestro therefore
exposes two versioned contracts, an Operations API (commands, entry points) and
an event stream read from the kernel journal with durable cursors (exit points),
and runs third-party code only as supervised, sandboxed extension processes
speaking versioned protocols, each with its own least-privilege Cedar
principal. Extensions are catalog content, activated per machine; a broker
bridge (NATS JetStream, Kafka) is itself an extension, so team-scale fan-out
needs no core change. Design: [07](../architecture/07-extensibility.md).

Amended 2026-09-30: extensions may use **any implementation language**;
Maestro's own code stays Rust. Every extension action is an MCP tool call,
including event-triggered actions. MCP does not replace the durable event
stream/cursors/acknowledgments above. Engine/checker-owned hook-event schemas
are implicit in the hook point, not catalog resources or dependency edges.
[ADR-0022](0022-manifest-layout-v4-and-language-neutral-extensions.md) and
[S3 D14](../../specs/003-catalog/plan.md#d14-backends-preferences-and-extension-boundaries)
define verified code inventories/releases, typed contracts, local secret/runtime
bindings and separately scoped digest-bound egress grants. S3 checks/installs
without launching content; S4 qualifies containment, supervision and delivery.
No automatic language/runtime installation or in-process ABI is introduced.

## Considered options

- Native dynamic libraries loaded by the core: no stable Rust ABI, and a faulty
  plugin crashes or corrupts the authority. Rejected.
- MCP as the only extension protocol: good for agent tools, but it has no durable
  at-least-once delivery with cursors and acknowledgements. Kept for tools only.
- An embedded message broker in the core: another service on every laptop for a
  need most installations do not have. A bridge extension covers it.
- WebAssembly components: strong in-process isolation; deferred until an
  extension needs in-process latency.

## Consequences

Event schemas are public API: additive changes within a major version, CI
compatibility tests, a deprecation window for each new major.
