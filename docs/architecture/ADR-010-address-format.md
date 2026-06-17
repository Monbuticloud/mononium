# ADR-010: Address Format

**Status:** Accepted

**Context:** Addresses are the primary user-facing identifier. They must be human-readable, error-resistant, and simple to parse.

**Decision:** Raw hex (0x-prefixed) with 8-byte BLAKE3 checksum.

```
Format:  0x + 32-bytes (64 hex) + 8-byte checksum (16 hex)
Example: 0x3a1b2c3d...[32 bytes]...a7f2b1c9d3e4f5a6
                                                      ^-- checksum
```

- Address bytes: BLAKE3-256 of Ed25519 public key
- Checksum: first 8 bytes of BLAKE3(address_bytes)

**Consequences:**

- Self-validating (catches typos without external libraries)
- Simple to parse and format
- No Bech32 dependency
- Long strings (80 hex chars) — trade-off for simplicity
