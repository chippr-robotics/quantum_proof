# Verification Strategy

## Scope Separation

| Property                         | Status   |
|----------------------------------|----------|
| Classical arithmetic correctness | PROVEN   |
| Reversible consistency           | PROVEN   |
| Ancilla cleanup (return to 0)   | PROVEN   |
| Resource count accuracy          | PROVEN   |
| Cost attribution accuracy        | PROVEN   |
| Jacobian ↔ affine equivalence   | PROVEN   |
| Curve parameter integrity        | PROVEN   |
| QFT gate correctness (small n)   | PROVEN   |
| Shor's end-to-end recovery       | PROVEN   |
| Quantum superposition behavior   | ASSUMED  |
| Hardware execution success       | UNKNOWN  |

## Multi-Layer Verification

### Layer 1: Unit Tests (91 tests across 4 crates)

Every reversible gate, arithmetic operation, and Shor sub-system is independently tested:

**goldilocks-field** (20 tests):
- Field axioms: addition/multiplication identity, commutativity, distributivity
- Additive and multiplicative inverses
- Subtraction (including underflow wrapping)
- Edge cases: p-1 + 1 = 0, (p-1)^2 = 1
- Exponentiation and Legendre symbol
- 7 property-based tests (proptest) verifying field axioms over random inputs

**ec-goldilocks** (10 tests):
- Infinity as identity element for point addition
- Scalar multiplication by zero returns infinity
- Point negation (finite and infinity)
- Jacobian scalar multiplication matches affine reference (10 scalar values)
- Jacobian mixed addition matches affine point addition
- Jacobian doubling matches affine doubling
- On-curve verification for generator and scalar multiples
- Double-scalar multiplication [a]G + [b]Q consistency

**reversible-arithmetic** (7 tests):
- NOT gate is self-inverse
- CNOT gate: control-dependent flipping, self-inverse property
- Toffoli gate: dual-control behavior, self-inverse property
- Quantum register load/read roundtrip (64-bit values)
- Register clean check (ancilla return to |0>)
- Resource counter accuracy (Toffoli/CNOT/NOT counting)
- All gate types are provably self-inverse

**group-action-circuit** (54 tests):
- QFT resource estimates (3 tests: single/dual/small register)
- QFT gate generation (6 tests: counts match estimates, gate sequence structure, inverse negates phases, offset correctness, dual-register + measurement)
- QFT classical simulation (6 tests: basis state |0⟩ → uniform, inverse recovers state, gate-by-gate matches direct DFT for 3 and 4 qubits, unitarity preservation)
- QuantumGate QASM export (4 tests: reversible gates, Hadamard/CR/Swap/Measure formatting, gate counts, qubit indices)
- Continued fractions (11 tests: mod_inverse, direct secret recovery, multi-measurement recovery, CF convergents for 7/3, 355/113, Fibonacci, gcd)
- Measurement simulation (4 tests: Shor relation c+kd≡0, batch generation, d nonzero, c in range)
- End-to-end Shor's (6 tests: circuit build, classical verification, various secrets k∈{1,2,7,42,100,255}, zero secret, summary format, gate count consistency)
- QASM export (3 tests: group-action export, full Shor export with QFT+measurement, stats JSON)
- Group-action integration (2 tests: classical correctness on multiple (a,b) pairs, linearity [a]G+[b]Q=[a+kb]G)

### Layer 2: Integration Tests

- Reversible point addition matches `ec-goldilocks::point_add` on random inputs
- Reversible double-scalar map matches `ec-goldilocks::double_scalar_mul`
- [a]G + [b]Q computed reversibly matches classical computation
- Jacobian and affine circuit builders produce equivalent results

### Layer 3: Classical Ground Truth

For each ECDLP instance on the Oath curves:
1. Generate random (a, b, k), set Q = [k]G
2. Compute [a]G + [b]Q via classical reference
3. Execute the reversible circuit on the same basis-state inputs
4. Verify outputs match
5. Cross-check: verify [a + k*b]G equals the result
6. Pollard's rho independently solves Q -> k on a subset

### Layer 4: Curve Parameter Verification

All Oath-N curve parameters are independently verified via SageMath's SEA (Schoof-Elkies-Atkin) algorithm:
1. Prime field verification
2. Non-singular discriminant
3. Group order recomputed from scratch (independent of generation script)
4. Prime order (no cofactor)
5. Non-anomalous (order != field characteristic, prevents Smart's attack)
6. Embedding degree > 4 (prevents MOV/Weil transfer)
7. Generator on-curve and full-order
8. Hasse bound satisfied

This runs as a CI workflow (`curve-verification.yml`) on every push, creating a tamper-proof audit trail with SHA-256 fingerprints of all parameter files.

### Layer 5: ZK Proof (SP1 Groth16)

- The SP1 guest program executes the circuit inside the zkVM
- Every arithmetic operation in the execution trace is proven correct
- The Groth16 SNARK (~200 bytes) is verifiable by anyone
- Published with verification key for independent verification

**The proof certifies**: correct execution on sampled basis-state inputs,
deterministic circuit construction, and exact resource counts.

**The proof does NOT certify**: quantum superposition behavior or that Shor's
algorithm will succeed on quantum hardware.

## Continuous Integration

Five GitHub Actions workflows enforce correctness on every push and PR:

| Workflow | What it verifies |
|----------|-----------------|
| `unit-tests.yml` | Per-crate unit tests (debug + release) + proptest with 1024 cases/property |
| `functional-tests.yml` | Workspace integration, dependency-chain validation (field → EC → gates → circuit), benchmark execution, SP1 compilation |
| `code-quality.yml` | `rustfmt` formatting, `clippy` linting (strict `-D warnings` on core crates), `cargo doc` build |
| `benchmark.yml` | Full benchmark suite with resource tables, QASM export, scaling projections; results in PR job summary |
| `curve-verification.yml` | SageMath SEA verification of all Oath-N parameters; SHA-256 fingerprints in job summary |

The dependency-chain job runs crates in order — goldilocks-field, then ec-goldilocks,
then reversible-arithmetic, then group-action-circuit — validating that algebraic
guarantees compose correctly across layers.

## Running Verification

```bash
# All 91 tests across 4 core crates
cargo test --workspace

# Per-crate testing
cargo test -p goldilocks-field        # 20 tests
cargo test -p ec-goldilocks           # 10 tests
cargo test -p reversible-arithmetic   # 7 tests
cargo test -p group-action-circuit    # 54 tests

# Classical ground truth (requires Sage-generated Oath params)
cargo test -p ec-goldilocks -- --test-threads=1

# Benchmark suite (resource counting + scaling projections + cost attribution)
cargo run --release -p benchmark

# QASM export
cargo run --release -p benchmark -- export-qasm

# Full pipeline with ZK proof
./scripts/full_pipeline.sh
```
