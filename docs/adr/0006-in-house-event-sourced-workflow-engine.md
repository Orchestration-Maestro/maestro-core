# An in-house, event-sourced workflow engine

Status: accepted, 2026-09-23.

Workflow graphs (agents, steps, gates, routers, fan-out and joins, bounded
loops) execute in an engine inside `maestro-runtime` whose state is a fold of
events in the kernel journal. The hard parts are Maestro-specific (broker
decisions, contract acceptance, uncertain side effects, reviewer independence)
and a laptop should not run another server, so a general durable-execution
platform would add a service without removing that work. Dynamic plans from the
orchestrator are graphs too, compiled and approved like reviewed ones.

## Considered options

- Restate (Rust runtime and SDK): strong durable execution, but a server per
  machine; reconsidered if runs ever span machines.
- Temporal: heavier, Rust SDK not the primary one.
- LangGraph-style crates (graph-flow, weavegraph): young, and their persistence
  and semantics would be replaced anyway.

## Consequences

Crash, replay and uncertain-effect tests are part of the S4 exit; the engine
borrows proven ideas (state reducers, checkpoints, interrupts) rather than code.
