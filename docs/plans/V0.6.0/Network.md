---
tags: [network, p2p, deployment]
---

# Network

## Network Tiers

Mononium operates across 4 network tiers, each with separate genesis, chain ID, and peer discovery:

```mermaid
graph LR
    Localnet --> Devnet
    Devnet --> Testnet
    Testnet --> Mainnet
```

| Tier     | Chain ID | Purpose                 | Validators |
| -------- | -------- | ----------------------- | ---------- |
| Localnet | 0        | Single-node dev         | 1          |
| Devnet   | 1        | Multi-validator testing | 3+         |
| Testnet  | 2        | Public test network     | Community  |
| Mainnet  | 3        | Production              | Public     |

## Network Configuration

Each network differs in:

- **Genesis file** — initial accounts, stakes, parameters
- **Chain ID** — replay protection between networks
- **Bootstrap peers** — seed nodes for P2P discovery
- **Consensus parameters** — may vary (e.g., Testnet could have faster block times)

## P2P Layer

Mononium uses **libp2p** (rust-libp2p) with:

- **Gossipsub** for block, transaction, vote, and evidence propagation
- **Kademlia** for peer discovery after bootstrap
- **Identify** protocol for peer metadata
- **mDNS** for localnet auto-discovery

### Topics

Four gossipsub topics, each scoped by chain_id:

| Topic                          | Message Type               | Purpose                |
| ------------------------------ | -------------------------- | ---------------------- |
| `mononium/txs/{chain_id}`      | `Vec<Transaction>` (SCALE) | Mempool propagation    |
| `mononium/blocks/{chain_id}`   | `Block` (SCALE)            | New block announcement |
| `mononium/votes/{chain_id}`    | `CommitVote` (SCALE)       | Consensus votes        |
| `mononium/evidence/{chain_id}` | `Evidence` (SCALE)         | Slashing evidence      |

### Gossip Message Limits

Each topic has per-peer size and rate limits to prevent bandwidth exhaustion and spam. Validation happens **before** gossip propagation — oversized or over-rate messages are dropped at the topic level without reaching higher-level handlers.

| Topic                          | Max message size | Rate limit (per peer) | Rationale                                |
| ------------------------------ | ---------------- | --------------------- | ---------------------------------------- |
| `mononium/txs/{chain_id}`      | 1 MB             | 20 msg/s              | Txs are user-generated, unbounded        |
| `mononium/blocks/{chain_id}`   | 500 KB           | 1 msg/s               | 1 proposer per 5s slot — no burst needed |
| `mononium/votes/{chain_id}`    | 1 KB             | 100 msg/s             | 21 validators max, ~22 B per vote        |
| `mononium/evidence/{chain_id}` | 5 KB             | 5 msg/s               | Rare event — no throughput required      |

**Size validation:** Applied to the raw SCALE bytes before deserialization. A block payload > 500 KB is dropped immediately and the sending peer's score is decremented by -5 (see [Peer Scoring](#peer-scoring)).

**Rate limit enforcement:** If a peer exceeds the per-topic msg/s limit, excess messages are dropped and the sending peer's score is decremented by -2 per violation. Rate is measured as a sliding window over the last second (simple rolling counter, no external crate needed).

**Topic-level size limits vs. consensus block cap:** The 500 KB block limit at the gossip layer enforces the protocol-level block size hard cap. A proposer CANNOT gossip a block larger than 500 KB — it is rejected before any consensus handler sees it. This prevents oversized blocks from wasting verification resources.

### Ports

| Service              | Default Port | Flag          | Notes                                 |
| -------------------- | ------------ | ------------- | ------------------------------------- |
| P2P (libp2p)         | **30333**    | `--p2p-port`  | Peer-to-peer networking               |
| JSON-RPC (WebSocket) | **9944**     | `--rpc-port`  | Transaction submission, subscriptions |
| REST (HTTP)          | **9933**     | `--rest-port` | Balance queries, block lookups        |

Following Polkadot convention — well-known to blockchain operators.

### Peer Discovery

```bash
# Start node with bootstrap peers (config file approach)
mononium-cli node --config configs/node.devnet.yaml

# Or via CLI flags (overrides config)
mononium-cli node \
  --genesis configs/genesis.devnet.json \
  --key my-validator \
  --bootnodes /ip4/1.2.3.4/tcp/30333/p2p/Qm...
```

1. Node connects to specified bootstrap peers via their multiaddrs
2. Kademlia discovers additional peers sharing the same chain_id
3. mDNS handles localnet auto-discovery (no bootnodes needed)
4. Identify protocol exchanges version and peer metadata

### Transport Compression

libp2p's built-in **snappy** compression is enabled at the transport layer. This compresses wire bytes transparently without affecting consensus hashing (blocks are always hashed uncompressed).

## Sync Protocol

Nodes synchronize the chain via a dedicated libp2p **Request-Response** protocol (separate from gossipsub). A node entering the network enters **sync mode** first — it does not participate in consensus until caught up to the chain tip.

### Design Principles

1. **No explicit sync sessions.** The chain's `parent_hash` links are the continuity mechanism. A disconnected node reconnects to any peer and requests blocks from its last verified hash. The peer either serves blocks building on that hash, or returns empty.
2. **Batch verification with rolling hash.** Each `BlockSyncResponse` includes a rolling BLAKE3 hash over the batch (see [ADR-018](../../architecture/ADR-018-sync-batch-hash.md)) for fast fork detection before full block verification.
3. **Trust the chain, not the peer.** All blocks are fully verified (parent_hash chain, signature, state root) before acceptance. A peer that fails verification is disconnected and may be scored down.
4. **Stateful cursor, stateless requests.** The node persists its sync position (`last_verified_height`, `last_verified_hash`, `target_height`) locally. Each request is self-contained — a peer statelessly serves whatever blocks it has.

### Sync State (Local Cursor)

The syncing node maintains a persistent local cursor. This cursor survives peer disconnections, process restarts, and network partitions:

```rust
struct SyncCursor {
    /// The height of the last fully verified block
    pub last_verified_height: u64,
    /// The hash of the last fully verified block — used as fork anchor for reconnection
    pub last_verified_hash: [u8; 32],
    /// The network tip height learned from peer announcements
    pub target_height: u64,
    /// Blocks currently being downloaded (not yet verified), stored by height range
    pub pending_range: Option<HeightRange>,
}

struct HeightRange {
    pub start: u64,
    pub end: u64,
    pub peer_id: PeerId,
}
```

The cursor is:

- **Created** when sync begins (set to genesis height 0, hash = genesis block hash)
- **Updated** each time a block batch is fully verified (`last_verified_height` and `last_verified_hash` advance)
- **Reset** on checkpoint load (set to checkpoint height and its verified state root hash)
- **Persisted** to disk after each verified batch (not in redb — a small file `{data_dir}/{chain_id}/sync_cursor.json`) to survive crashes. On restart, the node resumes from the persisted position. If the cursor file is missing or corrupted, the node falls back to full replay from genesis.

The cursor is not shared between peers. Each peer statelessly serves blocks on demand.

### Messages

**Pair 1: Height-based sync (catch-up from genesis, checkpoint, or gap)**

```rust
struct BlockSyncRequest {
    pub start_height: u64,                    // first block to request
    pub max_blocks: u16,                      // up to 500 (hard cap)
    pub direction: SyncDirection,
    pub known_block_hash: Option<[u8; 32]>,   // fork anchor — see disconnection handling below
}

enum SyncDirection {
    Forward,   // normal catch-up
    Backward,  // recent blocks from tip (for quick new-peer bootstrap)
}

struct BlockSyncResponse {
    pub blocks: Vec<Block>,
    pub highest_height: u64,                  // peer's current tip
    pub batch_hash: [u8; 32],                 // rolling BLAKE3 over this batch — see ADR-018
}
```

**`known_block_hash` behavior:**

The `known_block_hash` field serves as a **fork anchor**. The responding peer MUST:

1. Verify that `known_block_hash` exists in their canonical chain
2. Verify that `known_block_hash` is at height `start_height - 1`
3. If both pass → serve blocks `[start_height..]` building on `known_block_hash`
4. If either fails → return `blocks: []` with `highest_height` still set (the node tries a different peer)

If `known_block_hash` is `None`, the peer serves blocks unconditionally from `start_height`. This is only used for initial genesis sync where there is no verified anchor yet.

**`batch_hash` computation (per ADR-018):**

```rust
fn compute_batch_hash(genesis_hash: [u8; 32], blocks: &[Block]) -> [u8; 32] {
    let mut hash = genesis_hash;
    for block in blocks {
        hash = blake3::hash(&[hash.as_bytes(), block.hash().as_bytes()].concat());
    }
    hash
}
```

The syncing node computes the same rolling hash locally and compares. A mismatch means the peer served a different batch than expected — either the peer is on a different fork or the blocks were corrupted in transit.

**Pair 2: Hash-based sync (specific block provenance)**

```rust
struct BlockByHashRequest {
    pub block_hashes: Vec<[u8; 32]>,          // up to 100
}

struct BlockByHashResponse {
    pub blocks: Vec<Block>,                   // in request order, missing entries omitted
}
```

Used to fetch specific blocks by hash (e.g., after a fork is detected and the node needs blocks from a specific candidate chain for comparison).

**Pair 3: Checkpoint sync (fast bootstrap)**

```rust
struct CheckpointRequest {
    pub target_height: u64,                   // nearest checkpoint boundary
}

struct CheckpointResponse {
    pub height: u64,
    pub smt_nodes: Vec<(Vec<u8>, Vec<u8>)>,   // serialized SMT key-value pairs
    pub validator_set: Vec<ValidatorEntry>,    // full validator set at checkpoint era
    pub validator_set_hash: [u8; 32],
    pub checkpoint_block_header: BlockHeader,
    pub checkpoint_hash: [u8; 32],            // BLAKE3 of the entire checkpoint
}
```

**Checkpoint trust model:**

A checkpoint is trusted because the block at that height was committed via BFT (2/3+ validator signatures). The syncing node:

1. Uses the included `validator_set` (full validator entries with public keys) to know who signed
2. Verifies the BFT commit votes in `checkpoint_block_header` — 2/3+ signatures must match the validator set's public keys
3. Verifies the commit votes reference the checkpoint height
4. Rebuilds the SMT from `smt_nodes` → computes `computed_state_root`
5. Asserts `computed_state_root == checkpoint_block_header.state_root`
6. If any check fails → discard checkpoint, try a different peer

**Why `validator_set` is included in the response:**

The syncing node does not have historical state — it cannot reconstruct the era N validator set from genesis without replaying all blocks. Including the full set in `CheckpointResponse` allows verification without replay. The set is authenticated by (a) matching `validator_set_hash` (which is committed in the checkpoint meta) and (b) verifying the BFT commits against it.

Devnet nodes skip checkpoint sync entirely and always replay from genesis (small state, fast).

### Sync Flow (Complete)

The sync flow is divided into three phases: **discovery**, **checkpoint fast-forward** (skip if close to tip), and **block catch-up**.

```
╔══════════════════════════════════════════════════════════╗
║                   SYNC INIT PHASE                       ║
╚══════════════════════════════════════════════════════════╝

 1. Load sync cursor from disk (if exists)
    → If cursor exists and is valid: resume from last_verified_height + 1
    → If no cursor or corrupted: start from genesis (height 0)

 2. Connect to bootstrap peers (from config or mDNS)
    → Wait until at least 1 peer is connected (timeout: 30s)
    → If no peers after timeout → retry with exponential backoff (30s, 60s, 120s, cap at 300s)

 3. Learn network tip
    → Send BlockSyncRequest { direction: Backward, max_blocks: 1 }
    → Receive BlockSyncResponse { blocks: [latest_block], highest_height: T }
    → Set sync_cursor.target_height = T

 4. Determine sync path:
    gap = target_height - last_verified_height

    if gap <= (2 * ERA_LENGTH):       # ≤2 eras behind → fast replay
        go to BLOCK CATCH-UP phase
    elif gap > (2 * ERA_LENGTH):      # >2 eras behind → checkpoint fast-forward
        go to CHECKPOINT FAST-FORWARD phase

╔══════════════════════════════════════════════════════════╗
║              CHECKPOINT FAST-FORWARD PHASE               ║
╚══════════════════════════════════════════════════════════╝

 5. Find the nearest checkpoint boundary ≤ target_height:
    checkpoint_era = target_height / ERA_LENGTH
    checkpoint_height = checkpoint_era * ERA_LENGTH

 6. Request checkpoint:
    → Send CheckpointRequest { target_height: checkpoint_height }
    → Timeout: 30s (per peer)

 7. Verify checkpoint (see trust model above):
    → If valid:
        → Load SMT state from checkpoint data
        → Set sync_cursor.last_verified_height = checkpoint_height
        → Set sync_cursor.last_verified_hash = checkpoint_hash
        → Persist cursor to disk
        → Go to BLOCK CATCH-UP phase
    → If invalid or timeout:
        → Disconnect peer, try next peer
        → If all peers exhausted → fall back to full genesis replay

╔══════════════════════════════════════════════════════════╗
║                  BLOCK CATCH-UP PHASE                    ║
╚══════════════════════════════════════════════════════════╝

 8. While last_verified_height < target_height:
    a. Select a peer (round-robin through connected peers)
    b. Request next batch:
       → Send BlockSyncRequest {
           start_height: last_verified_height + 1,
           max_blocks: 100,
           direction: Forward,
           known_block_hash: Some(last_verified_hash),
         }
    c. Wait for response (timeout: 5s first attempt, 10s second, 15s third)

    d. If timeout (no response):
       → Mark peer as unresponsive (score -= 1)
       → Try next peer (same height range)
       → After 3 different peers all timeout → log warning, increase timeout to 15s

    e. On response received:
       → Compute local_batch_hash from received blocks
       → If local_batch_hash != response.batch_hash:
           → Fork detected or corrupt response
           → Disconnect peer, mark as bad (score -= 5)
           → Try next peer
       → If local_batch_hash matches:
           → Verify parent_hash chain:
               for i, block in enumerate(batch):
                   expected_parent = batch[i-1].hash if i > 0 else last_verified_hash
                   if block.parent_hash != expected_parent:
                       → Chain discontinuity — disconnect peer, try next peer
           → If parent_hash chain valid:
               → Verify and apply each block sequentially:
                   verify signature(block.proposer, block.header)
                   verify block.timestamp ± 2s
                   apply block transactions → compute state_root
                   assert computed_state_root == block.header.state_root
               → If any block fails verification:
                   → Entire batch rejected — disconnect peer, try next peer
               → If all blocks pass:
                   → Advance cursor: last_verified_height += batch.len()
                   → Update last_verified_hash to last block's hash
                   → Persist cursor to disk (after every batch)
                   → On new peer announcement of higher tip → update target_height

    f. If all connected peers exhausted:
       → Log warning, wait for new peers (kademlia discovery or mDNS)
       → Exponential backoff: 5s, 10s, 20s, cap at 60s
       → If no peers for 5 minutes → critical error, stay in sync mode

 9. Sync complete:
    → last_verified_height >= target_height
    → Exit sync mode, begin consensus participation
    → Cursor remains persisted for next restart or reconnection
```

### Disconnection Handling (Mid-Batch)

A peer disconnection mid-batch is handled without special session logic:

```
Scenario: Node requested blocks 50-149 from peer A. Peer A disconnects
         after sending blocks 50-72. Node has not yet verified them.

1. Node detects peer A disconnection (libp2p disconnect event)
2. Node discards the incomplete batch (blocks 50-72 not yet verified)
3. Node selects next peer (peer B)
4. Node sends BlockSyncRequest {
       start_height: 50,                    // same start — unverified
       max_blocks: 100,
       known_block_hash: Some(last_verified_hash),  // hash of block 49
   }
5. Peer B either serves blocks 50+ or returns empty (wrong fork)
6. Node verifies the complete batch from peer B

Key invariant: No block is ever marked as verified until the entire
batch passes parent_hash chain verification AND per-block state
verification. An incomplete batch is discarded without penalty.
```

No session state, no partial batch recovery. The `known_block_hash` anchor ensures peer B cannot serve a different fork without detection.

### Peer Disagreement Resolution (Fork During Sync)

```
Scenario: Node receives blocks from peer A at height 50-149. Batch hash
         matches. Parent_hash chain matches. But one block's state_root
         doesn't match after re-execution.

1. Verify each block in order, checking state_root after each
2. Block 78's state_root doesn't match → block 78 is invalid
3. Disconnect peer A (served invalid state transition)
4. Mark peer A's score -= 10 (significant penalty — served invalid data)
5. Request blocks 50-149 from peer B (same range, known_block_hash = block 49)
6. If peer B serves the same blocks and they verify OK at block 78:
   → peer A was on a different fork or serving corrupt data → confirmed bad
   → pin the peer A disconnect as evidence
7. If peer B serves DIFFERENT blocks at 50-149 (different batch_hash):
   → peer A and peer B disagree on the canonical chain at this height
   → request blocks from peer C (tiebreaker)
   → majority wins (2 of 3 peers agree → canonical is their chain)
   → minority peer scored down (score -= 5)
```

The tiebreaker (step 7) is rare — it requires two peers on different forks AND the syncing node at the exact fork height. Most of the time, one peer returns a matching batch_hash and the other doesn't, making the choice obvious.

### Retry Logic

| Attempt | Per-peer timeout | Action on failure                        |
| ------- | ---------------- | ---------------------------------------- |
| 1st     | 5s               | Try next peer, same height               |
| 2nd     | 10s              | Try next peer, same height               |
| 3rd     | 15s              | Try next peer, same height               |
| 4th     | —                | Log warning, wait 10s, retry from step 1 |

After 3 failures on different peers for the same height range, the node pauses 10 seconds before retrying the entire range from the last verified height. This prevents busy-looping on a fork or unavailable network.

After 10 consecutive batch failures across all peers for different height ranges, the node logs a critical error and stays in sync mode indefinitely, retrying with exponential backoff (10s, 30s, 60s, 120s, cap at 300s). Operator intervention is required.

### No-Peer Stall Handling

A node with no connected peers cannot sync. This is handled as a permanent retry loop — the node never gives up:

```
At startup:
 1. Attempt to connect to all configured bootnodes concurrently
 2. Start mDNS discovery (localnet only — no effect across subnets)
 3. If bootnodes list is empty AND no mDNS peers after 5s:
    → Retry mDNS: 5s → 10s → 30s → 30s (repeatedly)
    → Log warning on each cycle: "No bootnodes configured and no local peers discovered"
    → Node remains in sync mode, does not participate in consensus
 4. If bootnodes are configured but all fail to connect:
    → Retry bootnode connections on the same schedule: 5s → 10s → 30s → 30s
    → After 5 continuous minutes of zero connections → log critical error
    → Continue retrying indefinitely despite the error

After initial connection (all peers lost):
 5. If the last connected peer disconnects and no peers remain:
    → Return to step 3/4 behavior with the same retry schedule
    → The node was previously participating in consensus — it now falls behind
    → On reconnect → normal sync from persisted cursor

Key guarantee: The node retries forever. It never exits, panics, or shuts
down due to lack of peers. The operator is responsible for providing
reachable bootnodes or ensuring mDNS works (localnet only).
```

### Peer Scoring

A lightweight peer scoring system deters misbehavior and guides peer selection. Scores are per-peer, persisted in memory only (reset on node restart).

#### Score Tiers

| Score range | Status  | Behavior                                                                   |
| ----------- | ------- | -------------------------------------------------------------------------- |
| `> 0`       | Good    | Normal operation, preferred for sync requests                              |
| `-20 to 0`  | Neutral | Deprioritized for sync requests, still connected                           |
| `< -20`     | Banned  | Disconnected and banned for 1 era (banned list wiped at each era boundary) |

#### Score Adjustments

| Event                                                 | Score change | Rationale                                      |
| ----------------------------------------------------- | ------------ | ---------------------------------------------- |
| Valid block propagated                                | +1           | Good citizen                                   |
| Valid vote propagated                                 | +1           | Good citizen                                   |
| Successful sync batch served                          | +2           | Useful peer                                    |
| Sync batch hash mismatch (ADR-018)                    | -10          | Fork disagreement or corrupted data            |
| Sync batch fails verification (state_root mismatch)   | -20          | Served invalid state — unambiguous misbehavior |
| Empty sync response (peer has blocks but won't serve) | -2           | Wasting request slots                          |
| Timeout on sync request (2+ consecutive)              | -4           | Unresponsive                                   |
| Invalid block gossiped                                | -10          | Wasting bandwidth                              |
| Invalid vote gossiped                                 | -10          | Wasting bandwidth                              |
| Connect/disconnect loop (>3 disconnects in 5 min)     | -10          | Connection flapping                            |
| Duplicate block gossip (>3 identical blocks)          | -2           | Bandwidth waste                                |

#### Ban Mechanics

- When a peer's score drops below -20, the node:
  1. Disconnects the peer immediately
  2. Adds their `PeerId` to the in-memory ban list
  3. Ignores all future connection attempts from that peer until the ban expires
- Ban expiry: at the **next era boundary** (height % ERA_LENGTH == 0), the entire ban list is cleared and all scores reset to 0
- The era used for ban expiry is the node's **latest known era** (`last_verified_height / ERA_LENGTH`)
- For a node with zero blocks (fresh genesis), there is no era yet — fall back to a 1-hour wall-clock ban as a temporary measure
- Banning is local-only. If the majority of the network considers a peer honest, that peer will still be connected by other nodes

#### Rationale for Values

- A single `state_root` mismatch (-20) immediately bans a peer — serving invalid state is never ambiguous
- Two batch hash mismatches (-10 each) or invalid blocks (-10 each) also trigger a ban
- Positive scores (+1/+2 for good behavior) are small relative to negatives — building trust is slow, losing it is fast
- The 1-era ban (~1 hour) is proportional: long enough to be meaningful, short enough that a misconfigured node can recover quickly
- Era-based expiry is deterministic across all nodes (unlike wall-clock timers) and aligns with the chain's natural reset cadence

#### Deprioritization (Neutral Peers)

Peers in the `-20 to 0` range remain connected but are:

- Skipped when selecting peers for sync batch requests (prefer Good peers)
- Still allowed to gossip blocks, votes, and transactions (they may still have valid data)
- Allowed to receive our gossip (we don't want to fork away from them if they're on a minority chain)

This prevents a single bad interaction from losing a potentially useful peer while still down-ranking them for future work.

A node exits sync mode and begins consensus participation when ALL of the following are true:

1. `last_verified_height >= target_height` (caught up to tip)
2. The last received block's timestamp is within `2 × block_time` of local wall clock (not syncing a stale tip)
3. At least 1 peer remains connected (can receive new blocks)
4. No pending block verification in progress

If any condition becomes false after exiting sync mode (e.g., a reorg causes the node to fall >2 eras behind), the node re-enters sync mode automatically. This is detected via:

- Gossiped blocks that reference a `parent_hash` the node doesn't have
- Peer announcements with `highest_height >> local_height`
- Gossiped `Status` messages (if implemented) showing a chain split

## Development Progression

```
Localnet (dev machine)
    → Devnet (3+ VPS)
        → Testnet (open to community)
            → Mainnet (production)
```

## Replay Protection

Every transaction includes the chain ID. A tx signed for Localnet (ID 0) cannot be replayed on Mainnet (ID 3). This is enforced at the state machine level.

---

**Related:** [Validators](plans/V0.6.0/Validators.md), [Protocol](plans/V0.6.0/Protocol.md), [Roadmap](plans/V0.6.0/Roadmap.md)
