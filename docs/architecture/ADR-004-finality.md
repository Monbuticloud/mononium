# ADR-004: Finality Mechanism

**Status:** Accepted

**Context:** After a block is proposed, validators need to agree it's canonical. The mechanism must provide fast finality and be simple to implement.

**Decision:** BFT commit per block for V1, GRANDPA later.

- Each block gets a commit round
- Validators verify and sign commit votes
- 2/3+ commits = block is final (appears in next block's header)
- Actual finality: 1-2 blocks (~5-10s)

**Consequences:**

- Fast finality (not probabilistic)
- Simple — one commit round per block
- GRANDPA can be added via DI if the validator set grows large
- Fork handling via slashing (equivocation)
