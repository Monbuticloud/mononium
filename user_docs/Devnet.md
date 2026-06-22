# Devnet Deployment Guide

## Hardware Requirements

| Tier | Validators | CPU | RAM | Disk | Network |
|------|-----------|-----|-----|------|---------|
| Localnet | 1 | 2 cores | 4 GB | 10 GB | Local |
| Devnet | 3-5 | 4 cores | 8 GB | 50 GB | 100 Mbps |
| Testnet | 10-25 | 8 cores | 16 GB | 200 GB | 1 Gbps |

## Docker Devnet

### Prerequisites

- Docker Engine 24+
- Docker Compose v2

### Quick Start

```bash
# 1. Generate validator keys
./docker/generate-keys.sh 3

# 2. Update genesis with generated keys
# Edit configs/genesis.devnet.json with the addresses from step 1

# 3. Start the network
docker compose -f docker/docker-compose.yml up -d --scale validator=3

# 4. Check logs
docker logs mononium-bootstrap -f
docker logs mononium-observer

# 5. Verify blocks
curl http://localhost:9933/health
curl http://localhost:9936/height
```

### Port Allocation

| Container | P2P | REST | RPC |
|-----------|-----|------|-----|
| Bootstrap | 30332 | 9932 | 9942 |
| Validator N | 30333+N | 9933+N | 9943+N |
| Observer | 30336 | 9936 | 9946 |

## Manual Devnet

### Bootstrap Node

```bash
# Generate bootstrap key
mononium-cli wallet keygen bootstrap

# Start bootstrap node
mononium-cli node \
  --config configs/node.devnet.yaml \
  --key bootstrap \
  --p2p-port 30332 \
  --rest-port 9932 \
  --rpc-port 9942
```

### Validator Node

```bash
# Generate validator key
mononium-cli wallet keygen validator-1

# Start validator, connecting to bootstrap
mononium-cli node \
  --config configs/node.devnet.yaml \
  --key validator-1 \
  --p2p-port 30333 \
  --rest-port 9933 \
  --rpc-port 9943 \
  --bootnode /ip4/BOOTSTRAP_IP/tcp/30332/p2p/BOOTSTRAP_PEER_ID
```

### Observer Node

```bash
mononium-cli node \
  --observer \
  --genesis configs/genesis.devnet.json \
  --p2p-port 30336 \
  --rest-port 9936 \
  --rpc-port 9946 \
  --bootnode /ip4/BOOTSTRAP_IP/tcp/30332/p2p/BOOTSTRAP_PEER_ID
```

## Genesis Configuration

```json
{
  "chain_id": 0,
  "genesis_time": "2026-06-20T00:00:00Z",
  "initial_accounts": {
    "abababababababababababababababababababababababababababababababab": "100000000000000000000000000000000000"
  },
  "initial_validators": [
    {
      "address": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      "stake": "50000000000000000000000000000000000"
    }
  ]
}
```

Balance format: decimal MOXX (1 MONEX = 10^32 MOXX)

## Troubleshooting

| Symptom | Likely Cause | Fix |
|---------|-------------|-----|
| Node won't start | Port conflict | Check `--p2p-port`/`--rest-port`/`--rpc-port` are unique |
| No peers discovered | Wrong bootnode address | Verify peer ID matches bootstrap's actual key |
| Node syncs but drops | Firewall blocking TCP | Open P2P port (default 30333) |
| "genesis already loaded" | Duplicate data dir | Use fresh `--data-dir` or delete old chain data |
