#!/usr/bin/env bash
# run_benchmarks.sh — Run resource counting and scaling projections
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Oathbreaker Benchmark Suite ==="
echo ""

cd "$PROJECT_DIR"
cargo run --release -p benchmark

echo ""
echo "=== Oathbreaker Scale ==="
echo "Score your quantum computer by which Oath curve it can crack:"
echo "  Oath-8   — Toy (proof of concept)"
echo "  Oath-16  — Near-term devices"
echo "  Oath-32  — Medium-term milestone"
echo "  Oath-64  — Full benchmark target"
