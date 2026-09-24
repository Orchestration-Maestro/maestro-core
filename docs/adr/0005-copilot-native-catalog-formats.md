# Copilot-native catalog formats are canonical

Status: accepted, 2026-09-23.

Agents are authored as Copilot custom agent profiles (`.agent.md` with YAML
frontmatter), skills as Agent Skills (`SKILL.md`), instructions and prompts in
Copilot's formats. The same files then work in Copilot, in Pi and in the Maestro
runtime, and installing the catalog on a host is a verified copy rather than a
translation layer to maintain. Maestro-specific metadata (owner, maturity,
requirements, contracts) goes in a `metadata:` block if Copilot tolerates it,
otherwise in a sidecar file; an S3 spike decides.

## Consequences

The earlier "no frontmatter" rule is dropped. Format changes in Copilot become
catalog migrations, caught by `maestro catalog check`.
