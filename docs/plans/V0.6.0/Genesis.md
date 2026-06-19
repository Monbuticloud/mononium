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

| Unit  | Value        | Notes                  |
| ----- | ------------ | ---------------------- |
| MONEX | 1            | Primary unit (display) |
| MOXX  | 10^−32 MONEX | Smallest unit, "moss"  |

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
    max_supply: U256,          // 10,000,000,000 MONEX in MOXX (10^10 × 10^32)
    ceiling_rate: f64,         // 0.035 = 3.5% annual ceiling
    headroom_rate: f64,        // 0.05 = 5% of headroom
}
impl SupplyPolicy for CappedInflation {
    fn block_reward(&self, height: u64) -> U256 { ... }
}

**Parameters:**

| Parameter      | Value                      | Notes                                                  |
|----------------|----------------------------|--------------------------------------------------------|
| Ceiling rate   | **3.5%**                   | Applied to effective max supply — upper bound           |
| Headroom rate  | **5.0%**                   | Applied to remaining headroom until cap                 |
| Formula        | `min(5% × headroom, 3.5% × effective_max)` | Whichever is lower wins per era          |
| Base cap       | **10,000,000,000 MONEX**   | Hard floor on minted supply                             |
| Effective max  | `10B + cap_refill_balance` | Recalculated at each era boundary                      |
| Burn coins     | Permanently destroyed      | No effect on cap                                       |

**Effective max supply:**

The total minting cap = `base_cap + cap_refill_balance`. The `cap_refill_balance` is the amount of MONEX held at the Cap-Refill address (`0x00..01`). Anyone can voluntarily send MONEX there — coins are a sink (irreversible).

**Applied at era boundaries:** At each era transition (block % 720 == 0), the consensus engine snapshots the Cap-Refill balance and recomputes the effective max and current supply. Block rewards for the next era use these updated values. This prevents mid-era supply changes from breaking block reward determinism.

**Formula (per block, recalculated each era):**

```

headroom = effective_max - current_supply
annual_reward = min(5% × headroom, 3.5% × effective_max)
block_reward = annual_reward / blocks_per_year

```

**Three-phase behavior:**

| Phase             | Supply minted     | Dominant term       | Behavior                     |
| ----------------- | ----------------- | ------------------- | ---------------------------- |
| Early (flat)      | 0 — 30% of cap    | 3.5%×effective_max  | Constant ~55.5 MONEX/block   |
| Mid (decaying)    | 30% — 90%         | 5%×headroom         | Rewards drop linearly         |
| Late (tapering)   | >90%              | 5%×headroom (tiny)  | Asymptotic approach to cap   |

**Crossover:** When `current_supply > 30% of effective_max` (~3B minted), the headroom term drops below the ceiling — from that point onward, rewards decay toward zero. No supply cliff at year 28.

**Example:**
```

Year 0: supply = 0, headroom = 10B, min(5%×10B, 3.5%×10B) = 350M/yr → 55.5/block
Year 5: supply = 1.75B, headroom = 8.25B, min(5%×8.25B, 3.5%×10B) = 350M/yr → still flat
Year 10: supply = ∼3.2B, headroom = ∼6.8B, 5%×6.8B = 340M < 350M → decaying starts
Year 20: supply = ∼5.9B, headroom = ∼4.1B, 5%×4.1B = 205M/yr → ∼32.5/block
Year 28: supply = ∼8.0B, headroom = ∼2.0B, 5%×2.0B = 100M/yr → ∼15.9/block (tapering, no cliff)

```

**Consequences:**
- No supply cliff — smooth asymptotic approach to cap
- Flat rewards for first ~10 years (predictable for validators)
- Cap-Refill contributions continuously raise the ceiling, extending the flat phase
- No mid-era surprises — deterministic within each era

Swappable via `ConsensusConfig { supply: Box<dyn SupplyPolicy> }`. The CLI config for Dev networks injects `FixedSupply`; the Mainnet config injects `CappedInflation`.

### Future Treasury (V2.0+)

A portion of inflation can be diverted to a treasury/development fund, governed by on-chain voting.

---

**Related:** [Protocol](plans/V0.6.0/Protocol.md), [Fees](plans/V0.6.0/Fees.md), [Consensus](plans/V0.6.0/Consensus.md)
```
