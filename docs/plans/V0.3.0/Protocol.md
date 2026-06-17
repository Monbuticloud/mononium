---
tags: [protocol, transactions, state]
---

# Protocol

## Serialization

| Layer                                 | Format                     | Why                              |
| ------------------------------------- | -------------------------- | -------------------------------- |
| **Wire protocol** (blocks, tx, votes) | SCALE (parity-scale-codec) | Compact, fast, derive-based      |
| **State storage** (redb rows)         | SCALE                      | Same as wire — consistent        |
| **RPC** (API responses)               | JSON (serde)               | Human-readable, standard clients |

```rust
// One struct, both formats via dual derives
#[derive(Encode, Decode, Serialize, Deserialize)]
pub struct Transaction {
    pub sender: [u8; 32],
    pub nonce: u64,
    // ... fields derive both SCALE + JSON serialization
}
```

## State Model

Account-based (not UTXO). Addresses map directly to account objects. State is committed via a Sparse Merkle Tree (see [State Root](#state-root)).

```
Account {
    address: [u8; 32],
    balance: U256,
    nonce: u64,
    code_hash: Option<[u8; 32]>,  // for future smart contracts
}
```

## Transaction Types (V1)

1. **Transfer** — Move MONEX between accounts
2. **Stake** — Lock MONEX to become/activate a validator
3. **Unstake** — Begin withdrawal from validator set (7-day cooldown)
4. **RegisterValidator** — Declare intent to validate

## Transaction Format (Sketch)

All protocol signatures use **Falcon-512** (666 bytes).

```
Transaction {
    chain_id: u64,           // replay protection
    nonce: u64,              // account nonce
    sender: [u8; 32],        // source address
    recipient: [u8; 32],     // destination address
    amount: U256,            // value transfer
    fee: U256,               // transaction fee
    tx_type: u8,             // transfer, stake, etc.
    payload: Option<Vec<u8>>, // additional data
    signature: [u8; 666],    // Falcon-512 signature
}
```

## Block Structure (Sketch)

```
Block {
    header: BlockHeader,
    transactions: Vec<Transaction>,
    votes: Vec<CommitVote>,
}

BlockHeader {
    height: u64,
    parent_hash: [u8; 32],
    state_root: [u8; 32],        // Sparse Merkle Tree root over accounts + validators + meta
    meta_root: [u8; 32],         // flat BLAKE3 hash of chain metadata (height, era, etc.)
    tx_root: [u8; 32],
    timestamp: u64,
    proposer: [u8; 32],
    chain_id: u64,
}
```

Note: No block-level compression. Blocks are always serialized as raw SCALE bytes. Transport-layer compression (snappy) is handled by libp2p.

## State Root

The state root is computed via a **256-depth Sparse Merkle Tree** using **BLAKE3** as the hash function.

### Namespaces

The SMT uses a single tree with 3 namespaces:

| Prefix | Namespace | Contents |
|--------|-----------|----------|
| `0x00` | Accounts  | `Address → (balance: U256, nonce: u64, code_hash: Option<[u8;32]>)` |
| `0x01` | Validators | `PublicKey → (stake: U256, status: u8)` |
| `0x02` | Meta      | Chain-global state: height, era, active set hash, chain_id, total supply |

Namespacing is implemented via key prefixing: account keys are stored as `0x00 ++ address`, validator keys as `0x01 ++ pubkey`, meta keys as `0x02 ++ key_id`.

### Implementation

```rust
// mononium-rust-lib/src/crypto/trie.rs
pub trait Trie {
    fn get(&self, key: &[u8]) -> Option<Vec<u8>>;
    fn insert(&mut self, key: &[u8], value: Vec<u8>);
    fn root(&self) -> [u8; 32];
    fn prove(&self, key: &[u8]) -> MerkleProof;  // for future light clients
}
```

The SMT is a custom implementation in `mononium-rust-lib`. No external trie dependency. The implementation only needs insert, get, root, and prove for V1.

## State Transition

```mermaid
graph LR
    subgraph Block
        TX1[tx 1]
        TX2[tx 2]
        TX3[tx 3]
    end
    TX1 --> SM[State Machine]
    TX2 --> SM
    TX3 --> SM
    SM --> S0[Pre-State SMT]
    S0 --> S1[Post-State SMT]
    S1 --> H[SMT Root Hash]
```

- Transactions are applied in order within a block
- Each tx is validated (signature, nonce, balance) before execution
- State root after block = SMT root committing to full state
- Re-execute any block → deterministic state

## Transaction Fees

Fee per transaction = **flat component** + **size component** + **optional tip**

```rust
pub struct HybridFee {
    pub flat_fee: U256,         // 0.00667 MONEX — minimum cost per tx
    pub per_byte_rate: U256,    // 0.000467 MONEX/byte — proportional to size
    // tip is set by sender as part of Transaction
}
```

| Component  | Value                | Purpose                              | Set by             |
| ---------- | -------------------- | ------------------------------------ | ------------------ |
| Flat fee   | **0.00667 MONEX**    | Minimum cost per tx (spam prevention) | Protocol parameter |
| Per-byte   | **0.000467 MONEX**   | Proportional to state/storage cost    | Protocol parameter |
| Min fee    | **0.0667 MONEX**     | Mempool entry threshold               | Protocol parameter |
| Tip        | User-defined         | Priority for block inclusion          | Sender             |

```rust
impl FeePolicy for HybridFee {
    fn calculate_fee(&self, tx: &Transaction) -> U256 {
        self.flat_fee + self.per_byte_rate * U256::from(tx.encoded_size()) + tx.tip
    }
}
```

These values are the same across all network tiers (Localnet, Devnet, Testnet, Mainnet). Swappable via `FeePolicy` trait.

## Genesis

### Genesis File Format

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
    {"address": "0x...", "balance": "10_000_000_000_000_000_000"}
  ],
  "validators": [
    {"address": "0x...", "public_key": "0x...", "stake": 0}
  ]
}
```

### Loading Logic

1. Node checks for existing redb database file
2. If database exists → genesis already applied, skip
3. If database doesn't exist → parse genesis JSON, build initial SMT, create block 0 (genesis block)
4. Genesis block hash = BLAKE3 of block 0 header — any peer with a different genesis rejects connections

### Genesis Files

| File | Network | Supply | Recipients |
|------|---------|--------|------------|
| `configs/genesis.localnet.json` | Localnet | 10 MONEX | 1 test key |
| `configs/genesis.devnet.json` | Devnet | 10 MONEX | 3-5 test keys |
| `configs/genesis.testnet.json` | Testnet | 100 MONEX | Community faucet |
| `configs/genesis.mainnet.json` | Mainnet | 0 MONEX | Fair launch via inflation |

## Token Supply

### V1 Dev (Localnet/Devnet/Testnet): Fixed Supply

All MONEX are minted at genesis. No inflation, no block rewards. Validators earn only transaction fees.

### V2 Mainnet: Mixed Supply (Inflation with Cap)

Mainnet starts at 0 total supply. MONEX is minted via block rewards with a capped maximum supply. This replaces `FixedSupply` via the `SupplyPolicy` trait.

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
```

### Future: Mixed Supply (V2.0+)

Inflation is introduced, capped at a maximum total supply. Part minted as block rewards, part as a treasury/development fund.

```rust
pub struct CappedInflation {
    max_supply: U256,
    annual_rate: f64,  // e.g. 0.02 = 2%
}
impl SupplyPolicy for CappedInflation {
    fn block_reward(&self, height: u64) -> U256 { ... }
}
```

Swappable via `ConsensusConfig { supply: Box<dyn SupplyPolicy> }`.

## Chain ID

Each network gets a unique chain ID to prevent replay attacks across networks:

| Network  | Chain ID |
| -------- | -------- |
| Localnet | 0        |
| Devnet   | 1        |
| Testnet  | 2        |
| Mainnet  | 3        |

---

**Related:** [Architecture](plans/V0.3.0/Architecture.md), [Consensus](plans/V0.3.0/Consensus.md), [Network](plans/V0.3.0/Network.md)
