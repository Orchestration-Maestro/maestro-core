# Cedar for tool and operation authorization

Status: accepted, 2026-09-23.

The broker evaluates every tool call and operation with Cedar (cedar-policy
4.13): principals are workflow nodes, actions are normalized operations,
resources are paths, hosts, tools and repositories, and the schema is generated
from the tool registry, including MCP tool descriptions. Cedar is formally
verified, analyzable and designed for exactly this question; policies stay
reviewed catalog data, and the same policies back the native Copilot hook.

## Considered options

Rego through regorus (more general, harder to analyze) and hand-written rules
(fast to start, impossible to audit at scale).
