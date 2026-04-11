# Verification Strategy

## Scope Separation

| Property                         | Status   |
|----------------------------------|----------|
| Classical arithmetic correctness | PROVEN   |
| Reversible consistency           | PROVEN   |
| Ancilla cleanup (return to 0)   | PROVEN   |
| Resource count accuracy          | PROVEN   |
| Jacobian ↔ affine equivalence   | PROVEN   |
| Quantum superposition behavior   | ASSUMED  |
| QFT + measurement recovery      | DEFERRED |
| Hardware execution success       | UNKNOWN  |

## Multi-Layer Verification

### Layer 1: Unit Tests (38 tests across 4 crates)

Every reversible gate and arithmetic operation is independently tested:

**goldilocks-field** (20 tests):
- Field axioms: addition/multiplication identity, commutativity, distributivity
- Additive and multiplicative inverses
- Subtraction (including underflow wrapping)
- Edge cases: p-1 + 1 = 0, (p-1)^2 = 1
- Exponentiation and Legendre symbol
- 7 property-based tests (proptest) verifying field axioms over random inputs

**ec-goldilocks** (8 tests):
- Infinity as identity element for point addition
- Scalar multiplication by zero returns infinity
- Point negation (finite and infinity)
- Jacobian scalar multiplication matches affine reference (10 scalar values)
- Jacobian mixed addition matches affine point addition
- Jacobian doubling matches affine doubling
- On-curve verification for generator and scalar multiples

**reversible-arithmetic** (7 tests):
- NOT gate is self-inverse
- CNOT gate: control-dependent flipping, self-inverse property
- Toffoli gate: dual-control behavior, self-inverse property
- Quantum register load/read roundtrip (64-bit values)
- Register clean check (ancilla return to |0>)
- Resource counter accuracy (Toffoli/CNOT/NOT counting)
- All gate types are provably self-inverse

**group-action-circuit** (3 tests):
- QFT resource estimates for single 64-qubit register
- QFT resource estimates for dual 64-qubit register (2x single)
- QFT resource estimates for small register (4-qubit)

### Layer 2: Integration Tests

- Reversible point addition matches `ec-goldilocks::point_add` on random inputs
- Reversible double-scalar map matches `ec-goldilocks::double_scalar_mul`
- [a]G + [b]Q computed reversibly matches classical computation
- Jacobian and affine circuit builders produce equivalent results

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

## Continuous Integration

Three GitHub Actions workflows enforce correctness on every push and PR:

| Workflow | What it verifies |
|----------|-----------------|
| `unit-tests.yml` | Per-crate unit tests (debug + release) + proptest with 1024 cases/property |
| `functional-tests.yml` | Workspace integration, dependency-chain validation (field → EC → gates → circuit), benchmark execution, SP1 compilation |
| `code-quality.yml` | `rustfmt` formatting, `clippy` linting (strict `-D warnings` on core crates), `cargo doc` build |

The dependency-chain job runs crates in order — goldilocks-field, then ec-goldilocks,
then reversible-arithmetic, then group-action-circuit — validating that algebraic
guarantees compose correctly across layers.

## Running Verification

```bash
# All 38 tests across 4 core crates
cargo test --workspace

# Per-crate testing
cargo test -p goldilocks-field
cargo test -p ec-goldilocks
cargo test -p reversible-arithmetic
cargo test -p group-action-circuit

# Classical ground truth (requires Sage-generated Oath-64 params)
cargo test -p ec-goldilocks -- --test-threads=1

# Benchmark suite (resource counting + Oath-N tiers)
cargo run --release -p benchmark

# Full pipeline with ZK proof
./scripts/full_pipeline.sh
```
