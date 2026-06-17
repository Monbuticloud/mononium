# DOX framework

- DOX is a performant AGENTS.md hierarchy installed here
- Agent must follow DOX instructions across any edits

## Core Contract

- AGENTS.md files are binding work contracts for their subtrees
- Work products, source materials, instructions, records, assets, and durable docs must stay understandable from the nearest applicable AGENTS.md plus every parent AGENTS.md above it

## Read Before Editing

1. Read the root AGENTS.md
2. Identify every file or folder you expect to touch
3. Walk from the repository root to each target path
4. Read every AGENTS.md found along each route
5. If a parent AGENTS.md lists a child AGENTS.md whose scope contains the path, read that child and continue from there
6. Use the nearest AGENTS.md as the local contract and parent docs for repo-wide rules
7. If docs conflict, the closer doc controls local work details, but no child doc may weaken DOX

## Project Overview

**Mononium** is a Layer 1 blockchain built in Rust.

### Repo structure

```
mononium/
├── Cargo.toml                # workspace root
├── AGENTS.md                 # ← this file
├── mononium-rust-lib/        # core blockchain library
├── mononium-cli/             # CLI binary (node + wallet)
├── mononium-gui/             # GUI binary (desktop app)
├── docs/
│   ├── architecture/         # ADRs — architectural decision records
│   └── plans/                # versioned planning docs (V0.x.x)
```

### Key technical decisions (see ADRs for details)

| Area               | Decision                            | ADR     |
| ------------------ | ----------------------------------- | ------- |
| Workspace          | 3-crate: lib + cli + gui            | ADR-001 |
| Validator election | Top-N by stake, Phragmén later (DI) | ADR-002 |
| Block production   | Round-robin, VRF later (DI)         | ADR-003 |
| Finality           | BFT commit per block, GRANDPA later | ADR-004 |
| Token supply       | Fixed devnets, mixed mainnet (DI)   | ADR-005 |
| Fees               | Per-byte + flat + tip (DI)          | ADR-006 |
| Database           | redb (DI for RocksDB later)         | ADR-007 |
| P2P                | libp2p (gossipsub, kademlia)        | ADR-008 |
| Serialization      | SCALE (wire) + JSON (RPC)           | ADR-009 |
| Address            | RawHex + BLAKE3 checksum            | ADR-010 |
| Mempool            | Tip → Time → Nonce                  | ADR-011 |
| RPC                | jsonrpsee + REST hybrid             | ADR-012 |
| CLI                | clap derive                         | ADR-013 |
| Eras               | 720 blocks, era 0 open              | ADR-014 |
| Genesis            | 10/10/100/0 distribution            | ADR-015 |

### Tech stack (standard Rust ecosystem)

- Async: tokio
- Error handling: anyhow (CLI) + thiserror (lib)
- Logging: tracing
- Testing: cargo-nextest, proptest

### Versioning

- **V0.x.x** = planning phase (docs only, no code)
- **V1.x.x** = development phase (building the chain)
- **V2.0.0** = first stable release

## Update After Editing

Every meaningful change requires a DOX pass before the task is done.

Update the closest owning AGENTS.md when a change affects:

- purpose, scope, ownership, or responsibilities
- durable structure, contracts, workflows, or operating rules
- required inputs, outputs, permissions, constraints, side effects, or artifacts
- user preferences about behavior, communication, process, organization, or quality
- AGENTS.md creation, deletion, move, rename, or index contents

## Hierarchy

- Root AGENTS.md is the DOX rail: project-wide instructions, global preferences, durable workflow rules, and the top-level Child DOX Index
- Child AGENTS.md files own domain-specific instructions and their own Child DOX Index
- Each parent explains what its direct children cover and what stays owned by the parent
- The closer a doc is to the work, the more specific and practical it must be

## Child Doc Shape

- Create a child AGENTS.md when a folder becomes a durable boundary with its own purpose, rules, responsibilities, workflow, materials, or quality standards
- Work Guidance must reflect the current standards of the project or user instructions; if there are no specific standards or instructions yet, leave it empty
- Verification must reflect an existing check; if no verification framework exists yet, leave it empty and update it when one exists

Default section order:

- Purpose
- Ownership
- Local Contracts
- Work Guidance
- Verification
- Child DOX Index

## Style

- Keep docs concise, current, and operational
- Document stable contracts, not diary entries
- Put broad rules in parent docs and concrete details in child docs
- Prefer direct bullets with explicit names
- Do not duplicate rules across many files unless each scope needs a local version
- Delete stale notes instead of explaining history
- Trim obvious statements, repeated rules, misplaced detail, and warnings for risks that no longer exist

## Closeout

1. Re-check changed paths against the DOX chain
2. Update nearest owning docs and any affected parents or children
3. Refresh every affected Child DOX Index
4. Remove stale or contradictory text
5. Run existing verification when relevant
6. Report any docs intentionally left unchanged and why

## User Preferences

When the user requests a durable behavior change, record it here or in the relevant child AGENTS.md

## Child DOX Index

| Path                          | Scope                                     |
| ----------------------------- | ----------------------------------------- |
| `docs/AGENTS.md`              | Documentation conventions across all docs |
| `docs/architecture/AGENTS.md` | ADR format and conventions                |
| `docs/plans/AGENTS.md`        | Planning docs structure and versioning    |
