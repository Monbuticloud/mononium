---
tags: [cryptography, security]
---

# Cryptography

## Signature Scheme

### Primary: Ed25519

- **Ed25519** is the default signing algorithm for V1
- Fast verification (~60k sigs/sec per core)
- Well-studied, constant-time implementations available
- Small signatures (64 bytes)
- Batch verification support for block validation

### Future: Falcon (Post-Quantum)

- Falcon is noted as a post-quantum alternative
- Deferred until post-quantum readiness is needed
- Ed25519 is adequate for V1

## Hashing

**BLAKE3** for all hashing needs:

- Block hashing
- State root computation
- Transaction root (Merkle tree)
- Address derivation
- Merkle proofs

BLAKE3 is chosen for its speed — significantly faster than SHA-256 with strong security guarantees.

## Key Derivation

- Signing key: Ed25519 private key (32 bytes)
- Verification key: Ed25519 public key (32 bytes)

## Address Format

`0x` + 32 bytes raw hex (64 chars) + 8 byte checksum (16 chars)

```
0x3a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1 __checksum__
```

| Component | Size                | Notes                                                     |
| --------- | ------------------- | --------------------------------------------------------- |
| Raw bytes | 32 bytes            | BLAKE3-256 hash of public key                             |
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

| Component              | Algorithm          |
| ---------------------- | ------------------ |
| Transaction signatures | Ed25519            |
| Block signatures       | Ed25519            |
| Consensus votes        | Ed25519            |
| Block hashing          | BLAKE3             |
| State root             | BLAKE3 Merkle tree |
| Tx root                | BLAKE3 Merkle tree |
| Address derivation     | BLAKE3 (TBD)       |

---

**Related:** [Validators](plans/V0.1.0/Validators.md), [Protocol](plans/V0.1.0/Protocol.md)
