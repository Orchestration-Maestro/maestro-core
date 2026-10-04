---
id: workflow:core/feature-delivery
name: feature-delivery
version: 1.2.0
schema: maestro-source/2
maturity: reviewed
rows: [architecture.L11]
workflows: [feature-delivery]
requires:
  - agent:core/planner
  - agent:core/worker
  - skill:core/spec-compliance
  - skill:core/security-review
nodes:
  plan: { kind: agent, agent: "agent:core/planner" }
  code: { kind: agent, agent: "agent:core/worker" }
  test: { kind: step }
  spec: { kind: agent, agent: "agent:core/reviewer", skill: "skill:core/spec-compliance", independent_of: [code] }
  security: { kind: agent, agent: "agent:core/reviewer", skill: "skill:core/security-review", independent_of: [code] }
  reviewed: { kind: join }
  approve: { kind: gate }
edges:
  - plan -> code
  - code -> test
  - test -> spec
  - test -> security
  - spec -> reviewed
  - security -> reviewed
  - reviewed -> approve
---

# Synthetic missing-reviewer refusal

Topology-only fixture. Compile inputs resolve start, terminal and session bindings;
C22b adds contracts, state, policies and budgets before source admission.
