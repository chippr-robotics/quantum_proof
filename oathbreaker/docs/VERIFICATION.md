# Verification Strategy

## Multi-Layer Verification

### Layer 1: Unit Tests

Every reversible gate and arithmetic operation is independently tested:
- Each gate is verified to be its own inverse
- Each arithmetic operation matches the non-reversible reference implementation
- Ancilla qubits are verified to return to |0⟩ after uncomputation

### Layer 2: Integration Tests

- Reversible point addition matches `ec-goldilocks::point_add` on random inputs
- Reversible scalar multiplication matches `ec-goldilocks::scalar_mul`
- Full Shor circuit output matches classical computation for random scalars

### Layer 3: Classical Ground Truth

For each ECDLP instance on the Oath-64 curve (k, Q = [k]G):
1. Generate random scalar k
2. Compute Q = [k]G using classical scalar multiplication
3. Solve the ECDLP Q → k' using Pollard's rho
4. Verify k == k'
5. Verify the Shor circuit's classical execution also produces Q from k

### Layer 4: ZK Proof (SP1 Groth16)

- The SP1 guest program runs the circuit inside the zkVM
- Every operation is proven correct
- The Groth16 SNARK (~200 bytes) is verifiable by anyone
- Published with verification key for independent verification

## Running Verification

```bash
# Unit + integration tests
cargo test --workspace

# Classical ground truth (requires Sage-generated Oath-64 curve params)
cargo test -p ec-goldilocks -- --test-threads=1

# Full pipeline with ZK proof
./scripts/full_pipeline.sh
```

## What the Proof Attests

"The prover possesses a complete Shor circuit which, when executed classically on scalar input k and generator G of the Oath-64 curve, produces output [k]G for all tested inputs. The circuit uses X qubits and Y Toffoli gates."
