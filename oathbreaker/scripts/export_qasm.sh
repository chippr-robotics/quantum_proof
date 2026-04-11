#!/usr/bin/env bash
# export_qasm.sh — Export the Shor circuit in OpenQASM 3.0 format
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== QASM Circuit Export ==="
echo ""

cd "$PROJECT_DIR"

# TODO: Once the circuit is fully implemented, this will invoke
# a binary that builds the circuit and calls shor_circuit::export::export_qasm().
#
# For now, print a placeholder message.
echo "QASM export awaiting full circuit implementation."
echo "Once complete, output will be written to: oathbreaker_oath64.qasm"
echo ""
echo "The exported QASM is compatible with:"
echo "  - Qiskit (IBM)"
echo "  - Cirq (Google)"
echo "  - Any OpenQASM 3.0 toolchain"
