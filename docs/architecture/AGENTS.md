# DOX: docs/architecture/

## Purpose

Architectural Decision Records (ADRs) — permanent records of technical decisions with context and consequences.

## Ownership

This doc owns all ADR files in this directory.

## Local Contracts

- Each ADR is a single `.md` file: `ADR-NNN-title.md`
- ADRs are numbered sequentially (ADR-001, ADR-002...)
- Template:

```markdown
# ADR-NNN: Title

**Status:** Accepted | Proposed | Deprecated

**Context:** Why this decision needed to be made.

**Decision:** What was decided.

**Consequences:** What this means for the project.
```

- Status values: Accepted (final), Proposed (under review), Deprecated (replaced by newer ADR)
- When an ADR is deprecated, the replacement ADR is noted in its Consequences

## Work Guidance

- Create a new ADR for every significant architectural decision
- DI patterns should be documented in the ADR if they're part of the decision
- Update ADR status when decisions change
- Refer to the ADR index in the root AGENTS.md

## Verification

- None yet

## Child DOX Index

No children.
