#!/usr/bin/env bash
# Stable entry point for the dependency-free benchmark protocol.
set -euo pipefail
cd "$(dirname "$0")/.."
command -v python3 >/dev/null || {
    echo "python3 is required to run the CPython comparison benchmarks" >&2
    exit 1
}
exec python3 benches/run.py "$@"
