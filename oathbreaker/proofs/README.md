# Proof Artifacts

This directory contains generated proof artifacts from SP1 zkVM execution.

## Files (after proof generation)

- `oath64_proof.bin` — Groth16 SNARK proof
- `oath64_vk.bin` — Verification key for independent verification
- `circuit_summary.json` — Resource counts committed in the proof

## How to Verify

```bash
# Using the SP1 host program
cargo run --release -p sp1-host -- verify proofs/oath64_proof.bin

# Or using the SP1 CLI
sp1 verify --proof proofs/oath64_proof.bin --vk proofs/oath64_vk.bin
```

## What the Proof Attests

The Groth16 SNARK proves that:

1. A reversible circuit of X qubits and Y Toffoli gates was constructed
2. The circuit implements the coherent group-action map [a]G + [b]Q on the Oath-64 curve
3. When executed classically on N random basis-state inputs (a, b), the circuit
   produces outputs matching the classical double-scalar multiplication reference
4. All ancilla qubits return to |0> after uncomputation
5. The circuit's structure matches the committed hash

## What the Proof Does NOT Attest

- Correctness over quantum superpositions (assumed via reversibility)
- That Shor's algorithm will succeed on quantum hardware
- That the QFT + measurement pipeline recovers k (deferred to v2)
