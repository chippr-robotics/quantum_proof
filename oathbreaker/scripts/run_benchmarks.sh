#!/usr/bin/env bash
# run_benchmarks.sh — Run resource counting, Oath-N tiers, and scaling projections
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Oathbreaker Benchmark Suite ==="
echo ""

cd "$PROJECT_DIR"
cargo run --release -p benchmark
