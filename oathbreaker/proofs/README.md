# Proof Artifacts

This directory contains generated proof artifacts from SP1 zkVM execution.

## Generating Proofs

```bash
# Classical verification (no SP1 toolchain required)
cargo run --release -p sp1-host -- --tier oath-8 --num-cases 10

# Full Groth16 proof (requires SP1 toolchain)
cargo run --release -p sp1-host --features sp1 -- \
    --tier oath-8 --num-cases 10 --mode prove --output-dir ../../proofs
```

## Files (after proof generation)

| File | Contents |
|------|----------|
| `proof.bin` | Groth16 SNARK proof (~260 bytes) |
| `vk.bin` | Verification key for independent verification |
| `circuit_summary.json` | Public values: resource counts + circuit hash |

## How to Verify

```bash
# Using the SP1 CLI
sp1 verify --proof proofs/proof.bin --vk proofs/vk.bin
```

## What the Proof Attests

The Groth16 SNARK proves that:

1. A reversible circuit of X qubits and Y Toffoli gates was constructed
   using Jacobian projective coordinates with windowed scalar multiplication
2. The circuit implements the coherent group-action map `[a]G + [b]Q`
   on the specified Oath-N curve
3. When executed classically on N random basis-state inputs (a, b), the
   circuit produces outputs matching the classical double-scalar
   multiplication reference
4. The circuit's structure matches the committed SHA-256 hash

## What the Proof Does NOT Attest

- Correctness over quantum superpositions (assumed via reversibility)
- That Shor's algorithm will succeed on quantum hardware
- That the QFT + measurement pipeline recovers k (deferred to v2)
