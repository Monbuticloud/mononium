# ADR-001: Cargo Workspace Structure

**Status:** Accepted

**Context:** The project needs clean separation between core blockchain logic, CLI interface, and GUI. A monorepo with multiple crates keeps everything in one repo while enforcing module boundaries.

**Decision:** 3-crate Cargo workspace:

- `mononium-rust-lib` — core blockchain logic. Zero knowledge of CLI or GUI.
- `mononium-cli` — CLI binary (node daemon + wallet). Depends on rust-lib.
- `mononium-gui` — GUI desktop app. Depends on rust-lib.

**Consequences:**

- Clear dependency graph: lib ← cli, lib ← gui
- No circular dependencies
- GUI can be added later without restructuring
- Shared types live in one place (the lib)

**DI Pattern:** The lib exposes traits for all swappable components. CLI and GUI inject concrete implementations.
