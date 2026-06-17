# ADR-016: Cryptographic Signature Scheme

**Status:** Accepted

**Context:** The original plan specified Ed25519 as the primary signing algorithm, with Falcon noted as a future post-quantum upgrade path. Upon review, this was reversed: Falcon-512 is now the V1 signing algorithm. This decision affects transaction format (signature size), block validation performance, key storage, address derivation, and validator record sizes.

**Decision:** Falcon-512 across all protocol operations.

| Item           | Size       | Notes                                |
| -------------- | ---------- | ------------------------------------ |
| Signature      | 666 bytes  | Every transaction, block, vote       |
| Public key     | 897 bytes  | On-chain in validator records        |
| Private key    | 1281 bytes | Derived from 48-byte seed at startup |
| Seed (entropy) | 48 bytes   | Generated via CLI, stored encrypted  |

**Constant-time requirement:** The Falcon-512 signing implementation must be constant-time (no secret-dependent branches or memory accesses). Key generation does not require constant-time (offline operation).

**Impact on existing ADRs:**

- **Supersedes ADR-010 (Address Format):** Address derivation remains `BLAKE3(public_key)[..32]`, but the public key input is now 897 bytes (Falcon-512) instead of 32 bytes (Ed25519). The checksum mechanism is unchanged.
- **Affects ADR-001 (Workspace):** The `crypto/` module in `mononium-rust-lib` implements Falcon-512 instead of Ed25519.
- **Affects ADR-006 (Fees):** Larger signature size increases average transaction size by ~600 bytes, raising total fee per tx proportionally.
- **Affects ADR-004 (Finality):** Falcon verification is ~10x slower than Ed25519, contributing to the 20s verification window.

**Key storage (new, not covered by prior ADRs):** Validator Falcon-512 keys are stored encrypted at rest using NaCl secretbox (XSalsa20-Poly1305) with a key derived via Argon2id (1 GiB memory, 4 iterations, 4 parallel). The `argon2` crate (pure Rust, RustCrypto) provides the KDF.

**Consequences:**

- **Positive:** Post-quantum secure from day one. No need for a future migration.
- **Positive:** Ed25519 and Falcon-512 share the same `SignatureScheme` trait interface — swapping between them requires no consensus changes.
- **Negative:** ~10x slower signature verification than Ed25519. Mitigated by batch verification and modest V1 throughput targets (100-200 TPS).
- **Negative:** Larger block sizes. A 500 KB block holds fewer Falcon-signed txs (~500 txs at ~1 KB/tx) compared to Ed25519 (~4000 txs at ~125 B/tx).
- **Negative:** Larger validator records on-chain (897-byte public key vs 32 bytes).
- **Negative:** No hardware support for Falcon acceleration (unlike Ed25519 which has CPU instructions on some architectures).
