#!/usr/bin/env bash
# Build the GraphCraft WASM module and copy it next to index.html.
#
# Requires `wasm-pack` on PATH:
#   cargo install wasm-pack
#
# Usage:
#   apps/web/build.sh                 # debug build
#   apps/web/build.sh --release
set -euo pipefail

cd "$(dirname "$0")/../.."

PROFILE="${1:---dev}"
case "$PROFILE" in
  --release|--dev|--profiling) ;;
  *) echo "usage: $0 [--release|--dev|--profiling]" >&2; exit 2 ;;
esac

echo "building graphcraft_web ($PROFILE) …"
wasm-pack build crates/web --target web --out-name graphcraft_web --out-dir ../../apps/web/pkg "$PROFILE"
echo
echo "built. serve with:"
echo "  (cd apps/web && python3 -m http.server 8000)"
echo "then open http://localhost:8000/"
