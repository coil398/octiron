#!/usr/bin/env bash
# Build an Octiron game (or all of them) to WASM under web/pkg/.
#
# Games live under examples/ (engine demos) and games/ (standalone works).
#
# Usage:
#   ./build.sh [name]              # dev build (default game: demo)
#   ./build.sh [name] --release    # release build with wasm-opt if available
#   ./build.sh all                 # dev build every game for the gallery
#   ./build.sh all --release       # release build every game
#
# Each game is emitted as web/pkg/<name>.js (+ _bg.wasm); the gallery page
# loads them by ?game=<name>.
set -euo pipefail
cd "$(dirname "$0")"

EXAMPLE=""
PROFILE="--dev"
for arg in "$@"; do
  if [[ "$arg" == "--release" ]]; then
    PROFILE="--release"
  elif [[ "$arg" != --* ]] && [[ -z "$EXAMPLE" ]]; then
    EXAMPLE="$arg"
  fi
done
EXAMPLE="${EXAMPLE:-demo}"

# Builds the crate at <path> (e.g. examples/demo or games/continent-sim).
# web/pkg is two levels up for both roots, so the same --out-dir works.
build_one() {
  local path="$1"
  local name
  name="$(basename "$path")"
  echo "building $path ($PROFILE) -> web/pkg/$name"

  wasm-pack build "$path" \
    "$PROFILE" \
    --target web \
    --no-typescript \
    --out-dir "../../web/pkg" \
    --out-name "$name"

  # Post-process with wasm-opt when available (release only).
  if [[ "$PROFILE" == "--release" ]] && command -v wasm-opt &>/dev/null; then
    local wasm_path="web/pkg/${name}_bg.wasm"
    local before
    before=$(wc -c < "$wasm_path")
    echo "  wasm-opt -Oz $wasm_path ..."
    wasm-opt -Oz "$wasm_path" -o "$wasm_path"
    local after
    after=$(wc -c < "$wasm_path")
    local pct=$(( (before - after) * 100 / before ))
    echo "  size: $(( before / 1024 ))KB -> $(( after / 1024 ))KB (-${pct}%)"
  fi
}

if [[ "$EXAMPLE" == "all" ]]; then
  for dir in examples/*/ games/*/; do
    [[ -d "$dir" ]] || continue
    build_one "${dir%/}"
  done
elif [[ -d "examples/$EXAMPLE" ]]; then
  build_one "examples/$EXAMPLE"
elif [[ -d "games/$EXAMPLE" ]]; then
  build_one "games/$EXAMPLE"
else
  echo "error: no game named '$EXAMPLE' under examples/ or games/" >&2
  echo "available:" >&2
  ls examples games >&2
  exit 1
fi

echo
if [[ "$PROFILE" == "--release" ]]; then
  echo "done (release). deploy with:  wrangler pages deploy web"
else
  echo "done. serve with:  ./serve.sh 8137   then open http://localhost:8137"
fi
