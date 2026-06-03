#!/usr/bin/env bash
# Screenshot the running app via headed Chromium under Xvfb, which gives real
# WebGPU/WebGL compositing (chrome-headless-shell can't composite WebGPU).
#
# Usage: ./tools/shot.sh [url] [outfile] [waitMs]
#   BACKEND=webgl ./tools/shot.sh   # force the WebGL2 fallback path
set -euo pipefail
cd "$(dirname "$0")/.."
URL="${1:-http://localhost:8137/}"
OUT="${2:-tools/shot.png}"
WAIT="${3:-4000}"
exec xvfb-run -a env HEADED=1 BACKEND="${BACKEND:-webgpu}" node tools/shot.mjs "$URL" "$OUT" "$WAIT"
