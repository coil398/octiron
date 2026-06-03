#!/usr/bin/env bash
# Build an Octiron example (or all of them) to WASM under web/pkg/.
#
# Usage:
#   ./build.sh [example] [--release]   # build one example (default: demo)
#   ./build.sh all [--release]         # build every example for the gallery
#
# Each example is emitted as web/pkg/<example>.js (+ _bg.wasm); the gallery page
# loads them by ?game=<example>.
set -euo pipefail
cd "$(dirname "$0")"

EXAMPLE="${1:-demo}"
PROFILE="--dev"
for arg in "$@"; do
  if [[ "$arg" == "--release" ]]; then
    PROFILE="--release"
  fi
done

build_one() {
  local ex="$1"
  echo "building examples/$ex ($PROFILE) -> web/pkg/$ex"
  wasm-pack build "examples/$ex" \
    "$PROFILE" \
    --target web \
    --no-typescript \
    --out-dir "../../web/pkg" \
    --out-name "$ex"
}

if [[ "$EXAMPLE" == "all" ]]; then
  for dir in examples/*/; do
    build_one "$(basename "$dir")"
  done
else
  if [[ ! -d "examples/$EXAMPLE" ]]; then
    echo "error: examples/$EXAMPLE does not exist" >&2
    echo "available:" >&2
    ls examples >&2
    exit 1
  fi
  build_one "$EXAMPLE"
fi

echo
echo "done. serve with:  ./serve.sh 8137   then open http://localhost:8137"
