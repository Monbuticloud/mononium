---
tags: [cryptography, security]
---

# Cryptography

## Signature Scheme

### Primary: Falcon-512

- **Falcon-512** is the signing algorithm for all protocol operations
- **Crate:** Zcash [`falcon`](https://crates.io/crates/falcon) — pure Rust, constant-time verified, no C dependencies (see [ADR-019](../../architecture/ADR-019-crate-selection-crypto.md))
- NIST Level I security (≈ AES-128)
- Post-quantum secure — lattice-based
- Constant-time signing implementation required

### Key Sizes

| Item           | Size       | Notes                                            |
| -------------- | ---------- | ------------------------------------------------ |
| Seed (entropy) | 48 bytes   | Input to Falcon key generation                   |
| Private key    | 1281 bytes | Stored encrypted at rest                         |
| Public key     | 897 bytes  | On-chain in validator records, gossiped to peers |
| Signature      | 666 bytes  | Every transaction, block, and consensus vote     |

### Future: No Plan to Change

Falcon-512 is the permanent choice for V1. Post-quantum is already here — no need to "prepare" for it later. If Falcon-512 is ever broken, the `SignatureScheme` trait allows swapping without consensus changes.

## Hashing

**BLAKE3** for all hashing needs:

- Block hashing
- State root computation (Sparse Merkle Tree)
- Transaction root (Merkle tree)
- Address derivation
- Merkle proofs

BLAKE3 is chosen for its speed — significantly faster than SHA-256 with strong security guarantees.

## Key Derivation

- **Seed:** 48 random bytes (entropy for Falcon-512 keygen)
- **Private key:** Generated from seed via Falcon-512 key generation (~10ms, offline)
- **Public key:** 897 bytes, derived from private key

## Address Format

`0x` + 32 bytes raw hex (64 chars) + 8 byte checksum (16 chars)

```
0x3a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1 __checksum__
```

| Component | Size                | Notes                                                     |
| --------- | ------------------- | --------------------------------------------------------- |
| Raw bytes | 32 bytes            | BLAKE3-256 hash of **Falcon-512 public key**              |
| Checksum  | 8 bytes             | First 8 bytes of BLAKE3(address_bytes) — not the key hash |
| Display   | `0x` + 80 hex chars | 64 for address + 16 for checksum                          |

```rust
pub struct Address([u8; 32]);

pub fn format_address(addr: &Address) -> String {
    let checksum = blake3::hash(&addr.0).as_bytes()[..8];
    let hex = hex::encode(addr.0) + &hex::encode(checksum);
    format!("0x{}", hex)
}
```

The checksum catches typos without requiring a full Bech32 library. It's appended not interleaved — simple to parse, simple to validate.

## Protocol Use

| Component              | Algorithm          | Size      |
| ---------------------- | ------------------ | --------- |
| Transaction signatures | Falcon-512         | 666 bytes |
| Block signatures       | Falcon-512         | 666 bytes |
| Consensus votes        | Falcon-512         | 666 bytes |
| Block hashing          | BLAKE3             | 32 bytes  |
| State root             | BLAKE3 SMT         | 32 bytes  |
| Tx root                | BLAKE3 Merkle tree | 32 bytes  |
| Address derivation     | BLAKE3(pubkey)     | 32 bytes  |

## Key Storage

Validator keys are stored encrypted at rest:

| Component       | Mechanism                                                                              |
| --------------- | -------------------------------------------------------------------------------------- |
| **Encryption**  | NaCl secretbox (XSalsa20-Poly1305)                                                     |
| **KDF**         | Argon2id (512 MiB memory, 4 iterations, 4 parallel)                                    |
| **File format** | JSON: `{ "public_key": "0x...", "encrypted_seed": "base64...", "nonce": "base64..." }` |
| **Location**    | `~/.mononium/keys/{name}.json`                                                         |
| **Crate**       | `argon2` (pure Rust, RustCrypto)                                                       |

Key generation is an **offline CLI operation** (`mononium-cli wallet keygen`). The 48-byte seed is encrypted to disk; the private key is re-derived at node startup. The Argon2id memory cost (512 MiB) means ~2.5-5s unlock time — acceptable for a one-time startup operation.

---

**Related:** [Validators](plans/V0.7.0/Validators.md), [Protocol](plans/V0.7.0/Protocol.md)
