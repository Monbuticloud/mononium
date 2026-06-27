#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# Mononium profiling helpers
#
# Usage:
#   ./scripts/profile.sh sample     <seconds> [-- node-args...]
#   ./scripts/profile.s h benches   [--filter <bench-name>]
#
# Examples:
#   ./scripts/profile.sh sample 20 -- node --observer --genesis configs/genesis.devnet.json
#   ./scripts/profile.sh benches
#   ./scripts/profile.sh benches --filter "compute_tx_root/500"
#
# Notes:
#   - 'sample' uses macOS's built-in `sample` command. No extra tools.
#   - Output: profile.txt (call tree) + profile.svg (flamegraph, requires inferno)
#   - 'benches' runs criterion benchmarks.
# ---------------------------------------------------------------------------
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

BIN="mononium-cli"
PROFILE_NAME="flamegraph"         # defined in workspace Cargo.toml

# ---------------------------------------------------------------------------
# sample — macOS `sample` command (no sudo needed for the build, sudo for sample)
# ---------------------------------------------------------------------------
do_sample() {
    local duration="$1"
    shift

    local args=("$@")
    if [ "${args[0]:-}" = "--" ]; then
        args=("${args[@]:1}")
    fi

    echo "==> Building $BIN (profile: $PROFILE_NAME) ..."
    cargo build --profile "$PROFILE_NAME" --bin "$BIN" 2>&1 | grep -v "^warning:" | grep -v "^$" | grep -v "^   Compiling\|^   Finished" || true

    local BIN_PATH="target/$PROFILE_NAME/$BIN"
    if [ ! -f "$BIN_PATH" ]; then
        echo "ERROR: binary not found at $BIN_PATH"
        echo "       (build may have failed or profile name differs)"
        exit 1
    fi

    echo "==> Starting $BIN in background ..."
    echo "    Args: ${args[*]}"
    "$BIN_PATH" "${args[@]}" &
    local PID=$!

    # Wait for node to start (look for "consensus loop" in logs)
    sleep 3

    echo "==> Sampling PID $PID for ${duration}s ..."
    echo "    (may prompt for sudo — needed by macOS's sample(1))"
    sudo sample "$PID" "$duration" -file "$ROOT/profile.txt" 2>&1 || {
        echo ""
        echo "WARNING: sample failed. Common issues:"
        echo "  - Process exited before sampling finished"
        echo "  - SIP restricting sampling (check csrutil status)"
        echo "  Run manually: sudo sample <PID> $duration -file profile.txt"
    }

    # Stop the node
    echo "==> Stopping node (PID $PID) ..."
    kill "$PID" 2>/dev/null || true
    wait "$PID" 2>/dev/null || true

    # Check output
    if [ -f "$ROOT/profile.txt" ]; then
        local LINES
        LINES=$(wc -l < "$ROOT/profile.txt")
        echo "==> profile.txt written ($LINES lines)"

        # Try to generate SVG flamegraph
        if command -v inferno-flamegraph &>/dev/null; then
            echo "==> Generating flamegraph SVG (requires inferno)..."
            # `sample` output uses "Process: ..." header format
            # Convert sample output to folded stacks, then SVG
            < "$ROOT/profile.txt" \
              "$ROOT/scripts/sample-to-folded.awk" \
              | inferno-flamegraph > "$ROOT/profile.svg" 2>/dev/null && \
            echo "==> profile.svg written" || \
            echo "    (SVG generation skipped — sample output format may vary)"
        else
            echo "==> profile.txt ready (readable call tree)"
            echo "    Install inferno for SVG: cargo install inferno"
        fi
    else
        echo "==> No profile output. Try running the node manually and sampling:"
        echo "    sudo sample \$(pgrep mononium-cli) $duration -file profile.txt"
    fi
}

# ---------------------------------------------------------------------------
# benches — criterion benchmarks
# ---------------------------------------------------------------------------
do_benches() {
    local FILTER=""
    while [ $# -gt 0 ]; do
        case "$1" in
            --filter) FILTER="$2"; shift 2 ;;
            *) shift ;;
        esac
    done

    echo "==> Running mononium-lib benchmarks ..."
    if [ -n "$FILTER" ]; then
        cargo bench -p mononium-lib --bench "$FILTER"
    else
        cargo bench -p mononium-lib
    fi
}

# ---------------------------------------------------------------------------
# Main dispatch
# ---------------------------------------------------------------------------
case "${1:-help}" in
    sample)
        shift
        if ! [[ "${1:-}" =~ ^[0-9]+$ ]]; then
            echo "Usage: $0 sample <seconds> [-- node-args...]"
            echo ""
            echo "Examples:"
            echo "  $0 sample 20 -- node --observer --genesis configs/genesis.devnet.json"
            echo "  $0 sample 30 -- node --observer --genesis configs/genesis.localnet.json"
            exit 1
        fi
        do_sample "$@"
        ;;

    benches)
        shift
        do_benches "$@"
        ;;

    *)
        echo "Mononium profiling — usage:"
        echo ""
        echo "  $0 sample <sec> [-- node-args...]   Profile the node with macOS sample(1)"
        echo "    Outputs: profile.txt + profile.svg (if inferno installed)"
        echo ""
        echo "  $0 benches [--filter <name>]         Run criterion benchmarks"
        echo "    Outputs: timing results in terminal"
        echo ""
        echo "Examples:"
        echo "  $0 sample 20 -- node --observer --genesis configs/genesis.devnet.json"
        echo "  $0 sample 30 -- node --observer --genesis configs/genesis.localnet.json"
        echo "  $0 benches"
        echo "  $0 benches --filter compute_tx_root"
        exit 1
        ;;
esac
