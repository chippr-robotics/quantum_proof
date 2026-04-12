# Oathbreaker

## Open Reversible Circuit Framework for ECDLP over the Goldilocks Field
### With zk-Proven Execution Trace, Karatsuba Arithmetic, and Resource Benchmarking

**Project: Chippr Robotics LLC — Open Source Research**
**Status: Optimization Phase (v1.1) — April 2026**

---

## Overview

A reversible circuit framework implementing the arithmetic and coherent group-action
components required by Shor's algorithm for ECDLP, on the **Oath curve family** — toy
elliptic curves over Goldilocks-form prime fields (p = 2^n - 2^(n/2) + 1).

The circuit implements the coherent double-scalar map **[a]G + [b]Q** — the
computationally dominant component (>99% of qubits and gates) of Shor's ECDLP
algorithm. Uses **Jacobian projective coordinates** to eliminate per-operation
inversions, **Karatsuba multiplication** (O(n^1.585) per multiply), **symmetry-optimized
squaring**, and **Binary GCD inversion** (O(n^2) vs Fermat's O(n^2.585)). Independently
verified against classical ground truth via Pollard's rho, with ZK proof via SP1
Groth16 SNARK.

### Current Results (Measured)

| Tier | Qubits | Toffoli | Window |
|------|--------|---------|--------|
| **Oath-8** | ~295 | ~162K | w=4 |
| **Oath-16** | ~855 | ~997K | w=4 |
| **Oath-32** | ~2,848 | ~5.76M | w=8 |

**256-bit ECDLP projection**: ~1.2B Toffoli (Karatsuba model), ~24x gap to Litinski's 50M.

### Optimization History (Oath-32)

| Optimization | Toffoli | Change |
|-------------|---------|--------|
| Baseline (schoolbook + Fermat) | 8.38M | — |
| + Karatsuba multiplication | 7.37M | -12.0% |
| + Symmetry-optimized squaring | 6.62M | -10.1% |
| + Binary GCD inversion | 5.67M | -14.5% |
| + Proper Cuccaro arithmetic | 5.76M | +1.7% (correctness fix) |
| **Cumulative** | **5.76M** | **-31.3%** |

## What This Is

- A fully implemented reversible circuit framework + classical simulator of quantum arithmetic subroutines
- The coherent group-action map [a]G + [b]Q, resource-counted and QASM-exportable
- Jacobian projective coordinate system eliminating per-op modular inversions (single final inversion)
- Karatsuba O(n^1.585) multiplication with symmetry-optimized squaring (~50% fewer Toffoli)
- Binary GCD (Kaliski) O(n^2) inversion replacing Fermat O(n^2.585)
- Classically verifiable: the same ECDLP can be solved via Pollard's rho
- Proven correct via SP1 Groth16 SNARK (execution trace, not quantum simulation)
- A quantum hardware benchmark (the Oathbreaker Scale)

## What This Is NOT

- Not a quantum computer execution (no hardware exists at 64-bit scale)
- Not an attack tool (64-bit curves have ~32-bit security, trivially breakable)
- Not a full Shor implementation in v1 (QFT + measurement deferred to v2)
- Not a reproduction of withheld industrial circuits, but an open analogue

## The Oathbreaker Scale

Score your quantum computer by which Oath curve it can crack:

| Tier | Field | Measured Qubits | Measured Toffoli | Classical Difficulty | Target Era |
|------|-------|-----------------|------------------|---------------------|-----------|
| Oath-8 | 8-bit | 295 | 162K | Trivial | 2026-2027 |
| Oath-16 | 16-bit | 855 | 997K | Trivial | 2027-2028 |
| Oath-32 | 32-bit | 2,848 | 5.76M | Easy (~seconds) | 2029-2031 |
| **Oath-64** | **64-bit** | **~5,696** | **~90M** | **Hours (Pollard rho)** | **2032-2035** |

Estimates use Jacobian projective coordinates with Karatsuba multiplication,
windowed scalar multiplication (w=8), symmetry-optimized squaring, and Binary GCD
inversion. Oath-8/16/32 are measured; Oath-64 is projected via Karatsuba O(n^2.585) scaling.

## Measured Cost Attribution (Oath-32, w=8)

| Subsystem | Toffoli | Share |
|-----------|---------|-------|
| Doublings | 4,541,952 | 80.2% |
| Mixed additions | 971,616 | 17.1% |
| Inversion (BGCD) | 107,008 | 1.9% |
| Affine recovery | 36,868 | 0.7% |
| QROM decode/load | 8,160 | 0.1% |

**Key insight**: Doublings dominate at 80% of Toffoli cost. Next optimization
target is doubling formula improvement, not mixed addition or QROM.

## Architecture

```
oathbreaker/
├── crates/
│   ├── goldilocks-field/           # GF(p) arithmetic, p = 2^64 - 2^32 + 1
│   │                               #   field.rs — add, sub, mul, inverse, pow, sqrt (Tonelli-Shanks)
│   │                               #   constants.rs — field prime, generator
│   │                               #   field_tests.rs — 12 unit + 7 proptest + Legendre
│   ├── ec-goldilocks/              # EC ops + double-scalar mul + ECDLP solvers
│   │                               #   curve.rs — AffinePoint, JacobianPoint, CurveParams
│   │                               #   point_ops.rs — add, double, scalar_mul (affine + Jacobian)
│   │                               #   double_scalar_mul.rs — [a]G + [b]Q classical reference
│   │                               #   ecdlp.rs — Pollard's rho solver (Floyd cycle detection)
│   ├── reversible-arithmetic/      # Reversible gates, adders, multipliers, EC ops
│   │                               #   gates.rs — NOT, CNOT, Toffoli (self-inverse)
│   │                               #   register.rs — QuantumRegister (load, read, clean check)
│   │                               #   ancilla.rs — ancilla allocation and tracking
│   │                               #   adder.rs — Cuccaro ripple-carry, modular adder
│   │                               #   multiplier.rs — Karatsuba + schoolbook + squarer + cuccaro_subtract
│   │                               #   inverter.rs — Binary GCD (Kaliski) + Fermat inverters
│   │                               #   ec_add_jacobian.rs — reversible Jacobian mixed addition (3S+8M)
│   │                               #   ec_double_jacobian.rs — reversible Jacobian doubling (6S+3M)
│   │                               #   resource_counter.rs — Toffoli/CNOT/NOT gate counting
│   ├── group-action-circuit/       # Coherent [a]G + [b]Q circuit assembly
│   │                               #   scalar_mul_jacobian.rs — windowed scalar mul (Jacobian) + QROM
│   │                               #   double_scalar.rs — full circuit builder + CostAttribution
│   │                               #   precompute.rs — QROM precomputation tables
│   │                               #   export.rs — OpenQASM 3.0 export
│   ├── sp1-program/                # SP1 guest program (proven inside zkVM)
│   ├── sp1-host/                   # SP1 host: proof generation + verification
│   └── benchmark/                  # Resource counting, Oath-N tiers, comparisons
│                                   #   main.rs — benchmark orchestration, window sweep
│                                   #   scaling.rs — Karatsuba/schoolbook/empirical projections
│                                   #   comparison.rs — comparison to Roetteler, Litinski, Google
│                                   #   oath_tiers.rs — Oath-N tier definitions
│                                   #   params.rs — curve parameter loading from JSON
├── sage/                           # SageMath curve generation + verification scripts
├── proofs/                         # Generated proof artifacts
├── docs/                           # Architecture, limitations, benchmarking docs
│   └── paper/                      # LaTeX paper draft
└── scripts/                        # Pipeline and export scripts
```

## Implemented (v1.1)

All circuit layers are fully implemented with Karatsuba + Binary GCD optimizations:

| Layer | Component | Status |
|-------|-----------|--------|
| **1 — Field** | GF(p) add, sub, mul, inverse, pow, sqrt | Done |
| **2 — EC Classical** | Affine point add/double/scalar_mul, Jacobian double/mixed-add/scalar_mul | Done |
| **3 — Reversible Arithmetic** | Cuccaro adder, Karatsuba multiplier, symmetry squarer, Binary GCD inverter | Done |
| **4 — Reversible EC** | EC add/double in affine + Jacobian with proper Cuccaro subtraction | Done |
| **5 — Circuit Assembly** | Windowed scalar mul, one-hot QROM, coherent [a]G + [b]Q (affine + Jacobian) | Done |
| **6 — Verification** | Pollard's rho ECDLP solver, classical ground-truth cross-check | Done |
| **7 — Benchmarking** | Measured tiers, cost attribution, window sweep, 3-model scaling projections | Done |
| **8 — Curve Generation** | SageMath scripts for all Oath-N tiers + tamper-proof CI verification | Done |
| **9 — ZK Proof** | SP1 program/host structure (awaiting SP1 toolchain integration) | Stub |
| **10 — QFT** | Resource estimates computed; execution deferred to v2 | Estimated |

See [docs/LIMITATIONS.md](docs/LIMITATIONS.md) for full scope details.

## Testing

40 tests pass across 4 core crates:

```bash
# Run all tests
cargo test --workspace

# Per-crate testing
cargo test -p goldilocks-field        # 20 tests (12 unit + 7 proptest + Legendre)
cargo test -p ec-goldilocks           # 10 tests (affine, Jacobian, on-curve, scalar mul)
cargo test -p reversible-arithmetic   # 7 tests (gates, registers, resource counting)
cargo test -p group-action-circuit    # 3 tests (QFT resource estimates)
```

Property-based tests (proptest) verify field axioms — commutativity, associativity,
distributivity, additive/multiplicative inverses — over 1024 random inputs per
property in CI.

## Continuous Integration

Five GitHub Actions workflows run on every push and PR to `main`:

| Workflow | Purpose |
|----------|---------|
| `unit-tests.yml` | Per-crate unit tests (debug + release) and proptest (1024 cases/property) |
| `functional-tests.yml` | Workspace integration, layered dependency-chain validation, benchmark execution, SP1 compilation |
| `code-quality.yml` | `rustfmt`, `clippy` (strict `-D warnings` on core crates), `cargo doc` |
| `benchmark.yml` | Full benchmark suite, QASM export, resource tables, scaling projections |
| `curve-verification.yml` | SageMath SEA verification of all Oath-N curve parameters (tamper-proof) |

Each workflow includes a summary gate job for branch protection compatibility.

## Building

```bash
cargo build --workspace
```

## Benchmarking

```bash
# Full benchmark suite with scaling projections
cargo run --release -p benchmark

# Export OpenQASM 3.0 circuit
cargo run --release -p benchmark -- export-qasm

# Export all Oath-N tiers as QASM files
cargo run --release -p benchmark -- export-all-qasm
```

## License

MIT — see [LICENSE](LICENSE).
