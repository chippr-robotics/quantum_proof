#!/usr/bin/env bash
# export_qasm.sh — Export the group-action circuit in OpenQASM 3.0 format
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== QASM Circuit Export ==="
echo ""

cd "$PROJECT_DIR"

OUTPUT_FILE="${1:-oathbreaker_oath64.qasm}"

echo "Building and exporting Oath-64 group-action circuit..."
echo "  Target: OpenQASM 3.0"
echo "  Output: $OUTPUT_FILE"
echo ""

cargo run --release -p benchmark -- export-qasm "$OUTPUT_FILE"

echo ""
echo "The exported QASM describes the coherent group-action map:"
echo "  |a⟩|b⟩|O⟩ → |a⟩|b⟩|[a]G + [b]Q⟩"
echo ""
echo "Compatible with:"
echo "  - Qiskit (IBM)"
echo "  - Cirq (Google)"
echo "  - Any OpenQASM 3.0 toolchain"
