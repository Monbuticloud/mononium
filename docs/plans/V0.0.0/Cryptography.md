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
- Address: Derived from public key (to be specified — likely BLAKE3 hash prefix)

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

**Related:** [Validators](Validators.md), [Protocol](Protocol.md)
