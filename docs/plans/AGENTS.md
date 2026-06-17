# DOX: docs/plans/

## Purpose

Versioned planning documents for Mononium. Covers all design, architecture, and roadmap docs created during the planning phase.

## Ownership

This doc owns all versioned plan directories (`V0.0.0/`, `V0.1.0/`, etc.) and `Versioning.md`.

## Local Contracts

- Plan versions use folder naming: `V{semver}/` (e.g., `V0.0.0/`)
- V0.x.x = planning phase — semver does not apply, these are just labels
- V1.0.0+ = development and stable releases — semver applies
- Each plan version is a full snapshot of the docs at that point
- Cross-reference ADRs using `[[Vx.x.x/ADR-NNN]]` wikilinks

## Work Guidance

- Create new plan versions (V0.1.0, V0.2.0...) when the plan has meaningful changes
- Update `Versioning.md` when the version scheme changes
- Planning docs should link to the relevant ADRs where decisions are finalized

## Verification

- None yet

## Child DOX Index

No children. Plan versions are snapshots, not independently owned areas.
