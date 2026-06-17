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
    A --> F[[Storage]]
    A --> G[[Protocol]]
    A --> H[[Network]]
    A --> I[[Cryptography]]
    A --> J[[Roadmap]]
```

| Area            | Doc                     | Key Decisions                                 |
| --------------- | ----------------------- | --------------------------------------------- |
| 🧠 Philosophy   | [[V0.0.0/Philosophy]]   | Account-based, minimalism, portfolio project  |
| 🏗️ Architecture | [[V0.0.0/Architecture]] | Cargo workspace: lib + CLI + GUI              |
| 👥 Validators   | [[V0.0.0/Validators]]   | Cheap VPS target, PoS, lightweight            |
| ⚡ Consensus    | [[V0.0.0/Consensus]]    | PoS, 5s block, 20s finality                   |
| 💾 Storage      | [[V0.0.0/Storage]]      | ITTIA DB Lite, mutable + append-only tables   |
| 📋 Protocol     | [[V0.0.0/Protocol]]     | Account model, native tx first, state machine |
| 🌐 Network      | [[V0.0.0/Network]]      | Localnet → Devnet → Testnet → Mainnet         |
| 🔐 Cryptography | [[V0.0.0/Cryptography]] | Ed25519, BLAKE3, Falcon later                 |
| 🗺️ Roadmap      | [[V0.0.0/Roadmap]]      | 5 phases, benchmark early                     |

---

> **Next:** Start with [[V0.0.0/Philosophy]] to understand the design rationale.
