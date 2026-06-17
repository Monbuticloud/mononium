# ADR-015: Genesis Distribution

**Status:** Accepted

**Context:** Different network tiers need different initial token distributions.

**Decision:**

| Network  | Supply    | Recipients       | Notes                     |
| -------- | --------- | ---------------- | ------------------------- |
| Localnet | 10 MONEX  | 1 test key       | Single-node dev           |
| Devnet   | 100 MONEX | 3-5 test keys    | Multi-validator testing   |
| Testnet  | 100 MONEX | Community faucet | Public testing            |
| Mainnet  | 0 MONEX   | —                | Fair launch via inflation |

**Consequences:**

- Dev networks: trivial to set up, just give test keys some MONEX
- Mainnet: no pre-mine, no insider allocation
- Mainnet bootstraps via block rewards (ADR-005 supply policy)
- Genesis configs are JSON files per network tier
