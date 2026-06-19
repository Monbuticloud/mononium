# User-Facing Documentation (Phase 1)

## Scope

Create a `user_docs/` directory at the repository root containing operator-facing documentation for deploying and running a Mononium node. This is separate from `docs/` (which covers architecture and design decisions) — `user_docs/` is for people who want to run a validator, operate an RPC node, or spin up a devnet.

## Directory Structure

```
user_docs/
├── README.md          # Index + getting started
└── Devnet.md          # Local devnet deployment guide
```

Future additions (post-V1):

- `OperatorGuide.md` # Production validator operations
- `Troubleshooting.md` # Common issues and fixes
- `RPC.md` # RPC API reference (moved from Architecture.md)

## Devnet.md Requirements

| Section                      | Details                                                                                                                        | Phase |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ----- |
| **Hardware Requirements**    | CPU, RAM, disk (SSD), bandwidth for validators vs RPC-only nodes; devnet vs production tiers                                   | 1     |
| **Bootstrap Key Generation** | `mononium-cli key generate --scheme falcon-512`, backup mnemonic, public key format                                            | 1     |
| **Genesis Configuration**    | TOML genesis template, customizing MONEX allocation, bootstrap pubkeys, era 0 length, `max_validators`, `CappedInflation` rate | 1     |
| **Docker Compose**           | Multi-service compose file: bootstrap node, additional validators, RPC node; volumes, ports, networking                        | 1     |
| **Kubernetes (optional)**    | Basic StatefulSet + ConfigMap for k8s operators (lower priority)                                                               | 2     |
| **Monitoring**               | `--metrics-addr` flag, Prometheus scrape config, Grafana dashboard JSON, alerts for missed blocks                              | 1     |

## README.md Requirements

- Index linking to each user doc
- Quick-start section: clone → configure → run
- Cross-reference to `docs/` for architecture/design context

## Format

- Markdown, concise and operational
- Every config flag and CLI command includes the exact syntax
- Docker compose files as inline code blocks (extractable)
- Metrics/alert configs as inline code blocks

## Relationships

- `user_docs/README.md` is the entry point for all operator documentation
- `docs/plans/` cross-references `user_docs/` when planning documents reference deployment procedures
- `user_docs/Devnet.md` depends on CLI and node config implementations (Phase 1)
