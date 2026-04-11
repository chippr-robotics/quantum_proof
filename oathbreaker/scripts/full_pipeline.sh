#!/usr/bin/env bash
# full_pipeline.sh — End-to-end: generate curve → build → test → prove → verify
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

echo "=== Oathbreaker Full Pipeline ==="
echo "Coherent group-action circuit for ECDLP over the Oath-64 curve"
echo ""

# Step 1: Generate Oath-64 curve parameters (requires SageMath)
echo "[1/6] Generating Oath-64 curve parameters..."
if command -v sage &> /dev/null; then
    cd "$PROJECT_DIR/sage"
    sage generate_curve.sage
    sage verify_order.sage
    sage validate_params.sage
    cd "$PROJECT_DIR"
else
    echo "  SKIP: SageMath not installed. Using pre-generated oath64_params.json if available."
fi

# Step 2: Build all crates
echo "[2/6] Building workspace..."
cd "$PROJECT_DIR"
cargo build --workspace --release

# Step 3: Run tests
echo "[3/6] Running test suite..."
cargo test --workspace --release

# Step 4: Run benchmarks
echo "[4/6] Running benchmarks..."
cargo run --release -p benchmark

# Step 5: Generate SP1 proof
echo "[5/6] Generating SP1 Groth16 proof of execution trace..."
cargo run --release -p sp1-host

# Step 6: Export QASM
echo "[6/6] Exporting QASM circuit..."
"$SCRIPT_DIR/export_qasm.sh"

echo ""
echo "=== Pipeline Complete ==="
echo "Proof artifacts: proofs/"
echo "QASM export: oathbreaker_oath64.qasm"
