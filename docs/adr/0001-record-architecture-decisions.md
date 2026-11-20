# 0001. Record architecture decisions

- Status: Accepted
- Date: 2026-11-20
- Deciders: Orchestrator, maintainer

## Context
Skillet is being developed by several AI agents (different tools and models) plus humans.
Agents have no memory between sessions; decisions that live only in chat or commit messages get
re-litigated or silently reversed.

## Decision
Every hard-to-reverse decision (public API, syntax/semantics, `Value` layout, dependencies, MSRV,
wire formats, platform support) is recorded as an ADR in `docs/adr/`. Agents propose
(`Proposed`); the Orchestrator or a human accepts. Requirements live separately in `docs/prd/`.

## Consequences
- Agents must read relevant ADRs before changing an area (enforced by `AGENTS.md` §6).
- Some overhead per decision; mitigated by the short template.
