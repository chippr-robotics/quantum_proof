# Verification Strategy

## Scope Separation

| Property                         | Status   |
|----------------------------------|----------|
| Classical arithmetic correctness | PROVEN   |
| Reversible consistency           | PROVEN   |
| Ancilla cleanup (return to 0)   | PROVEN   |
| Resource count accuracy          | PROVEN   |
| Quantum superposition behavior   | ASSUMED  |
| QFT + measurement recovery      | DEFERRED |
| Hardware execution success       | UNKNOWN  |

## Multi-Layer Verification

### Layer 1: Unit Tests

Every reversible gate and arithmetic operation is independently tested:
- Each gate is verified to be self-inverse
- Each arithmetic operation matches the non-reversible reference implementation
- Ancilla qubits are verified to return to |0> after uncomputation

### Layer 2: Integration Tests

- Reversible point addition matches `ec-goldilocks::point_add` on random inputs
- Reversible double-scalar map matches `ec-goldilocks::double_scalar_mul`
- [a]G + [b]Q computed reversibly matches classical computation

### Layer 3: Classical Ground Truth

For each ECDLP instance on the Oath-64 curve:
1. Generate random (a, b, k), set Q = [k]G
2. Compute [a]G + [b]Q via classical reference
3. Execute the reversible circuit on the same basis-state inputs
4. Verify outputs match
5. Cross-check: verify [a + k*b]G equals the result
6. Pollard's rho independently solves Q -> k on a subset

### Layer 4: ZK Proof (SP1 Groth16)

- The SP1 guest program executes the circuit inside the zkVM
- Every arithmetic operation in the execution trace is proven correct
- The Groth16 SNARK (~200 bytes) is verifiable by anyone
- Published with verification key for independent verification

**The proof certifies**: correct execution on sampled basis-state inputs,
deterministic circuit construction, and exact resource counts.

**The proof does NOT certify**: quantum superposition behavior, that Shor's
algorithm will succeed on hardware, or that QFT recovery works.

## Running Verification

```bash
# Unit + integration tests
cargo test --workspace

# Classical ground truth (requires Sage-generated Oath-64 params)
cargo test -p ec-goldilocks -- --test-threads=1

# Full pipeline with ZK proof
./scripts/full_pipeline.sh
```
