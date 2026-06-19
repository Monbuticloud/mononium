# ADR-019: Crate Selection — Cryptography and Arithmetic

**Status:** Accepted

**Context:**

ADR-016 specifies Falcon-512 as the signing algorithm but does not name a specific crate. The Rust ecosystem has multiple Falcon implementations with different trade-offs in safety guarantees, build requirements, and maintenance maturity. Similarly, the plan requires U256 arithmetic with SCALE codec support, but no specific crate was selected.

Concrete crate names and versions are needed before Phase 1 implementation begins. These choices affect build time, binary size, audit surface, and long-term maintenance burden.

**Decision:**

### Falcon-512: Zcash `falcon` crate

| Decision | Value |
|---|---|
| Crate | [`falcon`](https://crates.io/crates/falcon) by Zcash Foundation |
| Version | `0.2`+ |
| License | MIT / Apache-2.0 |
| Rationale | Pure Rust, `no_std`, constant-time verified, maintained by Zcash Foundation security team |

Rejected alternatives:

| Crate | Why rejected |
|---|---|
| `pqcrypto-falcon` | Wraps C reference implementation via `libpqcrypto`. Requires C compiler in build chain. Contradicts "pure Rust" build requirement. Larger audit surface (C FFI). |
| `falcon-sig` | Community-maintained, less active, smaller ecosystem, no constant-time audit trail. |
| Self-implement | Unacceptable security risk. Falcon is complex to implement correctly. Use a battle-tested crate. |

### U256: `primitive-types`

| Decision | Value |
|---|---|
| Crate | [`primitive-types`](https://crates.io/crates/primitive-types) |
| Version | `0.12`+ |
| Features | `scale-codec` (SCALE encode/decode for U256) |
| License | MIT / Apache-2.0 |
| Rationale | Native `scale-codec` feature avoids manual SCALE impls. Used by Substrate/Polkadot ecosystem — battle-tested. `U256` is a built-in type. |

Rejected alternatives:

| Crate | Why rejected |
|---|---|
| `num-bigint` + `num-traits` | No SCALE support. Would require manual `Encode`/`Decode` impls. More general-purpose than needed. |
| `ethereum-types` | Heavier dependency tree. Includes types we don't need (H160, H256 variants). Pulls in `uint` crate indirectly. |

### KDF: `argon2` (RustCrypto)

| Decision | Value |
|---|---|
| Crate | [`argon2`](https://crates.io/crates/argon2) |
| Version | `0.5`+ |
| Features | `std` (parallelism via `rayon`) |
| License | MIT / Apache-2.0 |
| Rationale | Pure Rust, RustCrypto organization, well-audited. Single-purpose crate. |

### Symmetric Encryption: `chacha20poly1305` (RustCrypto)

| Decision | Value |
|---|---|
| Crate | [`chacha20poly1305`](https://crates.io/crates/chacha20poly1305) |
| Version | `0.10`+ |
| License | MIT / Apache-2.0 |
| Rationale | XChaCha20-Poly1305 is the authenticated encryption used by NaCl secretbox. RustCrypto implementation is pure Rust, well-maintained. |

### Other Crypto Crates

| Need | Crate | Version | License |
|---|---|---|---|
| BLAKE3 hashing | `blake3` | `1.5`+ | MIT / Apache-2.0 |
| Hex encoding | `hex` | `0.4`+ | MIT |
| Constant-time cmp | `subtle` | `2.6`+ | BSD-3-Clause |

**Consequences:**

**Positive:**
- All crypto crates are pure Rust — no C compiler needed, consistent build environment
- All selected crates are MIT / Apache-2.0 licensed — passes `cargo-deny` license checks trivially
- `primitive-types` with SCALE feature means `U256` can be used directly in `Encode`/`Decode` derives with zero boilerplate
- Zcash `falcon` crate has an existing constant-time audit trail — reduces security review scope

**Negative:**
- Zcash `falcon` crate is relatively young (first stable release 2024) — API may change between minor versions. Mitigated by pinning exact versions in `Cargo.toml`.
- `primitive-types` pulls in `uint` and `fixed-hash` crates as transitive deps — slightly wider dependency tree than `num-bigint`.

**Neutral:**
- If the `falcon` crate becomes unmaintained, the `SignatureScheme` trait (see Architecture.md) allows swapping to a different Falcon impl or even Ed25519 without consensus changes
- `subtle` is a tiny dependency (constant-time comparison) but worth calling out explicitly — it prevents timing attacks on signature comparison

**Related:** ADR-016 (superseded on crate selection), [Cryptography.md](../plans/V0.6.0/Cryptography.md), [Architecture.md](../plans/V0.6.0/Architecture.md)
