# DOX: docs/

## Purpose

All project documentation: architecture decisions (ADRs), planning docs, guides.

## Ownership

- `docs/architecture/` — ADRs owned by this doc
- `docs/plans/` — Planning docs, owned by `docs/plans/AGENTS.md`

## Local Contracts

- Keep docs concise, current, and operational
- Use Obsidian-friendly markdown (wikilinks, frontmatter tags)
- Every architectural decision gets an ADR in `docs/architecture/`

## Work Guidance

- ADRs use the standard template: Status, Context, Decision, Consequences
- Plan docs live in versioned folders: `docs/plans/V0.0.0/`, `docs/plans/V0.1.0/`
- V0.x.x = planning phase (docs only, no code)
- Cross-reference ADRs from plan docs where relevant

## Verification

- None yet

## Child DOX Index

| Path                     | Scope                                  |
| ------------------------ | -------------------------------------- |
| `architecture/AGENTS.md` | ADR format and conventions             |
| `plans/AGENTS.md`        | Planning docs structure and versioning |
