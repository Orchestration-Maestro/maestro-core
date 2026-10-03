// Default deny: only the ordinary public read has a fact-free permit.
@rule("default-deny")
permit(principal, action == Action::"read", resource == Target::"public");

// Destructive effects need independent user approval and host path facts.
@rule("destructive-operations")
permit(principal, action in [Action::"write", Action::"delete"], resource)
when { context.trusted && context has approved && context.approved &&
       context has protected && !context.protected };

@rule("destructive-operations")
@approval("required")
forbid(principal, action in [Action::"write", Action::"delete"], resource)
when { context.trusted && context has approved && !context.approved };

// A protected target always refuses, even with an approved effect.
@rule("protected-paths")
forbid(principal, action, resource)
when { context.trusted && context has protected && context.protected };

// Egress is denied unless the host's destination allowlist admits it.
@rule("egress-deny-by-default")
permit(principal, action == Action::"network", resource)
when { context.trusted && context has egress && context.egress };

// The name comes from the host, not a submitted tool argument.
@rule("mcp-allowlist")
permit(principal, action == Action::"mcp", resource)
when { context.trusted && context has tool && context.tool == "maestro.knowledge.search" };
