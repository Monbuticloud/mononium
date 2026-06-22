#!/usr/bin/env bash
# Generate N Falcon-512 key files for Docker validators.
# Usage: ./generate-keys.sh <count>
# Example: ./generate-keys.sh 5  →  keys/validator-1.json .. validator-5.json
set -euo pipefail

COUNT="${1:-3}"
KEYS_DIR="$(dirname "$0")/keys"
mkdir -p "$KEYS_DIR"

echo "Generating $COUNT validator keys in $KEYS_DIR/..."

for i in $(seq 1 "$COUNT"); do
    NAME="validator-$i"
    KEY_FILE="$KEYS_DIR/$NAME.json"
    if [[ -f "$KEY_FILE" ]]; then
        echo "  $NAME — already exists, skipping"
    else
        cargo run --quiet -p mononium-cli -- wallet keygen "$NAME"
        # Copy from ~/.mononium/keys/ to docker/keys/
        cp "$HOME/.mononium/keys/$NAME.json" "$KEY_FILE"
        echo "  $NAME — generated"
    fi
done

echo "Done — $(ls "$KEYS_DIR" | wc -l) key(s)"
ls -la "$KEYS_DIR"
