---
name: probe
description: Synthetic agent for the catalog host format probe.
tools: read, mcp:maestro/knowledge_search
extensions: {mcp_adapter}
model: probe/probe-child-model
skills: probe-skill
---

## Purpose

Answer questions about the public synthetic glossary. Marker: project-agent.

## Responsibilities

Search the synthetic collection before answering.

## Inputs

One question about the synthetic glossary.

## Working sequence

Search, read the evidence, answer with its citation.

## Outputs

One short answer that cites the evidence.

## Boundaries

Synthetic data only; no other tool.
