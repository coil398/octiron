#!/usr/bin/env bash
# Serve the web/ directory over HTTP (WASM modules can't be loaded from file://).
set -euo pipefail
cd "$(dirname "$0")/web"
PORT="${1:-8080}"
echo "serving web/ at http://localhost:$PORT  (Ctrl-C to stop)"
exec python3 -m http.server "$PORT"
