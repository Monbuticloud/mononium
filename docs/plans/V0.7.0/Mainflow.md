---
tags: [index, hub]
---

# Mononium — L1 Blockchain

**Mononium** is a Layer 1 blockchain built in Rust. Native token is **Monium (MONEX)**.

```mermaid
graph LR
    A[Mononium] --> B[[Philosophy]]
    A --> C[[Architecture]]
    A --> D[[Validators]]
    A --> E[[Consensus]]
    A --> EE[[Mempool]]
    A --> F[[Storage]]
    A --> G[[Protocol]]
    A --> GG[[Fees]]
    A --> GGG[[Genesis]]
    A --> H[[Network]]
    A --> I[[Cryptography]]
    A --> II[[Slashing]]
    A --> III[[Governance]]
    A --> J[[Testing]]
    A --> K[[Roadmap]]
```

| Area            | Doc                                          | Key Decisions                                               |
| --------------- | -------------------------------------------- | ----------------------------------------------------------- |
| 🧠 Philosophy   | [Philosophy](plans/V0.7.0/Philosophy.md)     | Account-based, minimalism, Falcon-512, redb                 |
| 🏗️ Architecture | [Architecture](plans/V0.7.0/Architecture.md) | Cargo workspace: lib + CLI + GUI, SMT state root            |
| 👥 Validators   | [Validators](plans/V0.7.0/Validators.md)     | Cheap VPS target, PoS, validator lifecycle                  |
| ⚡ Consensus    | [Consensus](plans/V0.7.0/Consensus.md)       | PoS, 5s block, 20s finality, BFT commit                     |
| 🗄️ Mempool      | [Mempool](plans/V0.7.0/Mempool.md)           | Tip → Time → Nonce, nonce buffering, rate limit             |
| 💾 Storage      | [Storage](plans/V0.7.0/Storage.md)           | redb, mutable + append-only, checkpoints                    |
| 📋 Protocol     | [Protocol](plans/V0.7.0/Protocol.md)         | Account model, Falcon-512 sigs, native tx first             |
| 💰 Fees         | [Fees](plans/V0.7.0/Fees.md)                 | Hybrid fee, anti-spam deposit, pro-rata distro              |
| 🌱 Genesis      | [Genesis](plans/V0.7.0/Genesis.md)           | JSON format, capped inflation, fair launch                  |
| 🌐 Network      | [Network](plans/V0.7.0/Network.md)           | libp2p gossipsub, 4 topics, snappy compression              |
| 🔐 Cryptography | [Cryptography](plans/V0.7.0/Cryptography.md) | Falcon-512, BLAKE3, Argon2id key storage                    |
| ⚖️ Slashing     | [Slashing](plans/V0.7.0/Slashing.md)         | 90% equivocation penalty + 72-era freeze, evidence format   |
| 🗳️ Governance   | [Governance](plans/V0.7.0/Governance.md)     | On-chain stake-weighted voting, 7-era window, param-mutable |
| 🧪 Testing      | [Testing](plans/V0.7.0/Testing.md)           | 5-tier pyramid, src/tests/ mirrors src/                     |
| 🗺️ Roadmap      | [Roadmap](plans/V0.7.0/Roadmap.md)           | 5 phases, benchmark early                                   |

---

> **Next:** Start with [Philosophy](plans/V0.7.0/Philosophy.md) to understand the design rationale.
