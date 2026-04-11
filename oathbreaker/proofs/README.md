# Proof Artifacts

This directory contains generated proof artifacts from SP1 zkVM execution.

## Files (after proof generation)

- `oathbreaker_proof.bin` — Groth16 SNARK proof
- `verification_key.bin` — Verification key for independent verification
- `circuit_summary.json` — Resource counts committed in the proof

## How to Verify

```bash
# Using the SP1 host program
cargo run --release -p sp1-host -- verify proofs/oathbreaker_proof.bin

# Or using the SP1 CLI
sp1 verify --proof proofs/oathbreaker_proof.bin --vk proofs/verification_key.bin
```

## What the Proof Attests

The Groth16 SNARK proves that:

1. A reversible circuit of X qubits and Y Toffoli gates was constructed
2. The circuit performs operations consistent with Shor's algorithm for ECDLP on the Oath-64 curve
3. When executed classically on N random scalar inputs, the circuit produces
   the correct EC point [k]G on the Oath-64 curve for each input
4. The circuit's structure matches the committed hash
