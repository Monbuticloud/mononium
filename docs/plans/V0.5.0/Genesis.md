---
tags: [protocol, genesis, supply]
---

# Genesis

## Genesis File Format

Each network tier has a JSON genesis file committed to the repo:

```bash
mononium-cli node --genesis configs/genesis.localnet.json
```

```json
{
  "chain_id": 0,
  "genesis_time": "2026-06-17T00:00:00Z",
  "initial_height": 0,
  "consensus": {
    "block_time_sec": 5,
    "era_length": 720,
    "max_validators": 21,
    "election_mode": "Open"
  },
  "accounts": [
    { "address": "0x...", "balance": "100000000000000000000000000000000000" }
  ]
  "bootstrap": {
    "public_key": "0x...",
    "blocks": 20
  }
}
```

### Loading Logic

1. Node checks for existing redb database file
2. If database exists → genesis already applied, skip
3. If database doesn't exist → parse genesis JSON, build initial SMT, create block 0 (genesis block)
4. Genesis block hash = BLAKE3 of block 0 header — any peer with a different genesis rejects connections

### Genesis Files

| File                            | Network  | Supply                       | Validators                                            |
| ------------------------------- | -------- | ---------------------------- | ----------------------------------------------------- |
| `configs/genesis.localnet.json` | Localnet | 10 MONEX (1 key)             | Bootstrap only (1 block)                              |
| `configs/genesis.devnet.json`   | Devnet   | 100 MONEX per key (3-5 keys) | Bootstrap (20 blocks) → era 0 Open                    |
| `configs/genesis.testnet.json`  | Testnet  | 100 MONEX                    | Bootstrap (100 blocks) → era 0 Open                   |
| `configs/genesis.mainnet.json`  | Mainnet  | 0 MONEX                      | Bootstrap (100 blocks) → era 0 Open + CappedInflation |

## Denomination

| Unit   | Value         | Notes                     |
| ------ | ------------- | ------------------------- |
| MONEX  | 1             | Primary unit (display)    |
| MOXX   | 10^−32 MONEX  | Smallest unit, "moss"     |

1 MONEX = 10^32 MOXX. All on-chain amounts are stored as MOXX (U256). Display formatting divides by 10^32.

Constants for code:
```rust
const ONE_MONEX: U256 = U256::from_str("100000000000000000000000000000000"); // 10^32
const ONE_MOXX: U256 = U256::one();
```

## Token Supply

### Dev Networks (Localnet/Devnet/Testnet): Fixed Supply

All MONEX are minted at genesis. No inflation, no block rewards. Validators earn only transaction fees.

### Mainnet: Capped Inflation

Mainnet starts at 0 total supply. MONEX is minted via block rewards with a capped maximum supply. Validators earn **transaction fees + block rewards**.

```rust
pub trait SupplyPolicy: Send + Sync {
    fn block_reward(&self, height: u64) -> U256;
}

pub struct FixedSupply;
impl SupplyPolicy for FixedSupply {
    fn block_reward(&self, _height: u64) -> U256 {
        U256::zero() // no inflation
    }
}

pub struct CappedInflation {
    max_supply: U256,     // 10,000,000,000 MONEX in MOXX (10^10 × 10^32)
    annual_rate: f64,     // 0.035 = 3.5%
}
impl SupplyPolicy for CappedInflation {
    fn block_reward(&self, height: u64) -> U256 { ... }
}

**Parameters:**

| Parameter | Value | Notes |
|-----------|-------|-------|
| Annual rate | **3.5%** | Applied to current effective max supply |
| Base cap | **10,000,000,000 MONEX** | Hard floor on minted supply |
| Effective max | `10B + cap_refill_balance` | Recalculated at each era boundary |
| Burn coins | Permanently destroyed | No effect on cap |

**Effective max supply:**

The total minting cap = `base_cap + cap_refill_balance`. The `cap_refill_balance` is the amount of MONEX held at the Cap-Refill address (`0x00..01`). Anyone can voluntarily send MONEX there — coins are a sink (irreversible).

**Applied at era boundaries:** At each era transition (block % 720 == 0), the consensus engine snapshots the Cap-Refill balance and recomputes the effective max. Block rewards for the next era use this updated value. This prevents mid-era supply changes from breaking block reward determinism.

**Formula (per block, constant within an era):**

```

block_reward = effective_max \* annual_rate / blocks_per_year

```

**Example:
```

Year 1: effective_max = 10B, block_reward ≈ 55.5 MONEX/block
Year 10: 3.5B minted, cap_refill = 100M → effective_max = 10.1B, block_reward ≈ 56.1 MONEX/block
Year 28: 10B minted, cap_refill = 500M → effective_max = 10.5B, inflation continues at adjusted rate

```

**Consequences:**
- Inflation naturally ends around year 28 (with no cap_refill)
- Cap-Refill contributions extend the tail gradually
- No mid-era surprises — deterministic within each era

Swappable via `ConsensusConfig { supply: Box<dyn SupplyPolicy> }`. The CLI config for Dev networks injects `FixedSupply`; the Mainnet config injects `CappedInflation`.

### Future Treasury (V2.0+)

A portion of inflation can be diverted to a treasury/development fund, governed by on-chain voting.

---

**Related:** [Protocol](plans/V0.5.0/Protocol.md), [Fees](plans/V0.5.0/Fees.md), [Consensus](plans/V0.5.0/Consensus.md)
