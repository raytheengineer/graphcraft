#!/usr/bin/env bash
# Build the GraphCraft eframe-web runner.
#
# Requires wasm-pack:
#   cargo install wasm-pack
#
# Usage:
#   apps/graphcraft-web/build.sh          # debug build
#   apps/graphcraft-web/build.sh --release

set -euo pipefail
cd "$(dirname "$0")/../.."

PROFILE="${1:---dev}"
case "$PROFILE" in
  --release|--dev|--profiling) ;;
  *) echo "usage: $0 [--release|--dev|--profiling]" >&2; exit 2 ;;
esac

echo "building graphcraft-web ($PROFILE) …"
wasm-pack build apps/graphcraft-web \
  --target web \
  --out-name graphcraft_web \
  --out-dir ../../apps/graphcraft-web/pkg \
  "$PROFILE"

echo
echo "built. serve with:"
echo "  (cd apps/graphcraft-web && python3 -m http.server 8000)"
echo "then open http://localhost:8000/"
