# DOX: docs/plans/

## Purpose

Versioned planning documents for Mononium. Covers all design, architecture, and roadmap docs created during the planning phase.

## Ownership

This doc owns all versioned plan directories (`V0.0.0/`, `V0.1.0/`, etc.) and `Versioning.md`.

## Local Contracts

- Plan versions use folder naming: `V{semver}/` (e.g., `V0.0.0/`)
- V0.x.x = planning phase — semver does not apply, these are just labels
- V1.0.0+ = development and feature completion phases — semver applies (partial in V1.x.x, full in V2.x.x)
- V3.0.0+ = stable release phase — full semver
- Each plan version is a full snapshot of the docs at that point
- Cross-reference ADRs using `[[Vx.x.x/ADR-NNN]]` wikilinks

## Work Guidance

- Create new plan versions (V0.1.0, V0.2.0...) when the plan has meaningful changes
- Update `Versioning.md` when the version scheme changes
- Planning docs should link to the relevant ADRs where decisions are finalized
- **Writing standard:** Be as explicit as possible. Every parameter, invariant, edge case, and state machine behavior must be spelled out in concrete terms. Avoid assumptions, implicit behaviors, and hand-waving. If a decision has trade-offs, document both the chosen path and the rejected alternatives with rationale. The plan must be unambiguous enough to implement from directly without needing to ask clarifying questions.

## Verification

- None yet

## Child DOX Index

| Path       | Scope                                                                          |
| ---------- | ------------------------------------------------------------------------------ |
| `GRILL.md` | Tracking doc for open questions, deferred decisions, and upcoming grill topics |

Plan version folders (`V0.0.0/`, `V0.1.0/`, etc.) are snapshots, not independently owned areas.
