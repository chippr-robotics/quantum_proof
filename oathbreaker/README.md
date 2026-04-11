# Oathbreaker

## Open Reversible Circuit Framework for ECDLP over the Goldilocks Field
### With zk-Proven Execution Trace and Resource Benchmarking

**Project: Chippr Robotics LLC — Open Source Research**
**Status: Implemented (v1) — April 2026**

---

## Overview

A reversible circuit framework implementing the arithmetic and coherent group-action
components required by Shor's algorithm for ECDLP, on the **Oath-64** curve — a
toy elliptic curve over the Goldilocks field (p = 2^64 - 2^32 + 1).

The circuit implements the coherent double-scalar map **[a]G + [b]Q** — the
computationally dominant component (>99% of qubits and gates) of Shor's ECDLP
algorithm. Uses **Jacobian projective coordinates** to eliminate per-operation
inversions, reducing gate cost by ~6x compared to affine. Independently verified
against classical ground truth via Pollard's rho, with ZK proof via SP1 Groth16 SNARK.

## What This Is

- A fully implemented reversible circuit framework + classical simulator of quantum arithmetic subroutines
- The coherent group-action map [a]G + [b]Q, resource-counted and QASM-exportable
- Jacobian projective coordinate system eliminating per-op modular inversions (single final inversion)
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

| Tier | Field | Est. Qubits | Est. Toffoli | Classical Difficulty | Target Era |
|------|-------|-------------|-------------|---------------------|-----------|
| Oath-8 | 8-bit | ~35 | ~8K | Trivial | 2026-2027 |
| Oath-16 | 16-bit | ~100 | ~120K | Trivial | 2027-2028 |
| Oath-32 | 32-bit | ~280 | ~3M | Easy (~seconds) | 2029-2031 |
| **Oath-64** | **64-bit** | **~700** | **~17M** | **Hours (Pollard rho)** | **2032-2035** |

Estimates use Jacobian projective coordinates with windowed scalar multiplication
and a single Fermat inversion at the end of the circuit.

## Architecture

```
oathbreaker/
├── crates/
│   ├── goldilocks-field/           # GF(p) arithmetic, p = 2^64 - 2^32 + 1
│   │                               #   field.rs — add, sub, mul, inverse, pow, sqrt (Tonelli-Shanks)
│   │                               #   constants.rs — field prime, generator
│   │                               #   field_tests.rs — 12 unit tests + 7 proptest properties
│   ├── ec-goldilocks/              # EC ops + double-scalar mul + ECDLP solvers
│   │                               #   curve.rs — AffinePoint, JacobianPoint, CurveParams
│   │                               #   point_ops.rs — add, double, scalar_mul (affine + Jacobian)
│   │                               #   double_scalar_mul.rs — [a]G + [b]Q classical reference
│   │                               #   ecdlp.rs — Pollard's rho solver (Floyd cycle detection)
│   │                               #   tests.rs — 8 tests (Jacobian, affine, on-curve checks)
│   ├── reversible-arithmetic/      # Reversible gates, adders, multipliers, EC ops
│   │                               #   gates.rs — NOT, CNOT, Toffoli (self-inverse)
│   │                               #   register.rs — QuantumRegister (load, read, clean check)
│   │                               #   ancilla.rs — ancilla allocation and tracking
│   │                               #   adder.rs — Cuccaro ripple-carry, modular adder
│   │                               #   multiplier.rs — schoolbook multiplier, squarer
│   │                               #   inverter.rs — Fermat (a^(p-2)) and binary GCD inverters
│   │                               #   ec_add_affine.rs — reversible EC point addition (affine)
│   │                               #   ec_double_affine.rs — reversible EC point doubling (affine)
│   │                               #   ec_add_jacobian.rs — reversible Jacobian mixed addition
│   │                               #   ec_double_jacobian.rs — reversible Jacobian doubling
│   │                               #   resource_counter.rs — Toffoli/CNOT/NOT gate counting
│   │                               #   tests.rs — 7 tests (gates, registers, resource counting)
│   ├── group-action-circuit/       # Coherent [a]G + [b]Q circuit assembly
│   │                               #   scalar_mul.rs — windowed scalar mul (affine)
│   │                               #   scalar_mul_jacobian.rs — windowed scalar mul (Jacobian)
│   │                               #   double_scalar.rs — full circuit builder (affine + Jacobian)
│   │                               #   precompute.rs — QROM precomputation tables
│   │                               #   qft_stub.rs — QFT resource estimates (deferred to v2)
│   │                               #   export.rs — OpenQASM 3.0 export
│   │                               #   tests.rs — 3 tests (QFT estimates)
│   ├── sp1-program/                # SP1 guest program (proven inside zkVM)
│   ├── sp1-host/                   # SP1 host: proof generation + verification
│   └── benchmark/                  # Resource counting, Oath-N tiers, comparisons
│                                   #   oath_tiers.rs — Oath-N tier definitions
│                                   #   comparison.rs — comparison to Roetteler, Litinski, Google
│                                   #   scaling.rs — n-bit to 256-bit scaling projections
│                                   #   counter.rs — circuit resource table printer
├── sage/                           # SageMath Oath-64 curve generation scripts
├── proofs/                         # Generated proof artifacts
├── docs/                           # Architecture, limitations, benchmarking docs
└── scripts/                        # Pipeline and export scripts
```

## Implemented (v1)

All circuit layers are fully implemented:

| Layer | Component | Status |
|-------|-----------|--------|
| **1 — Field** | GF(p) add, sub, mul, inverse, pow, sqrt | Done |
| **2 — EC Classical** | Affine point add/double/scalar_mul, Jacobian double/mixed-add/scalar_mul | Done |
| **3 — Reversible Arithmetic** | Cuccaro adder, modular adder, schoolbook multiplier, Fermat/GCD inverter | Done |
| **4 — Reversible EC** | EC add/double in affine + Jacobian projective coordinates | Done |
| **5 — Circuit Assembly** | Windowed scalar mul, QROM lookup, coherent [a]G + [b]Q (affine + Jacobian) | Done |
| **6 — Verification** | Pollard's rho ECDLP solver, classical ground-truth cross-check | Done |
| **7 — Benchmarking** | Oath-N tier definitions, prior work comparison, scaling projections | Done |
| **8 — ZK Proof** | SP1 program/host structure (awaiting SP1 toolchain integration) | Stub |
| **9 — QFT** | Resource estimates computed; execution deferred to v2 | Estimated |

See [docs/LIMITATIONS.md](docs/LIMITATIONS.md) for full scope details.

## Testing

38 tests pass across 4 core crates:

```bash
# Run all tests
cargo test --workspace

# Per-crate testing
cargo test -p goldilocks-field        # 20 tests (12 unit + 7 proptest + Legendre)
cargo test -p ec-goldilocks           # 8 tests (affine, Jacobian, on-curve)
cargo test -p reversible-arithmetic   # 7 tests (gates, registers, resource counting)
cargo test -p group-action-circuit    # 3 tests (QFT resource estimates)
```

Property-based tests (proptest) verify field axioms — commutativity, associativity,
distributivity, additive/multiplicative inverses — over 1024 random inputs per
property in CI.

## Continuous Integration

Three GitHub Actions workflows run on every push and PR to `main`:

| Workflow | Purpose |
|----------|---------|
| `unit-tests.yml` | Per-crate unit tests (debug + release) and proptest (1024 cases/property) |
| `functional-tests.yml` | Workspace integration, layered dependency-chain validation, benchmark execution, SP1 compilation |
| `code-quality.yml` | `rustfmt`, `clippy` (strict `-D warnings` on core crates), `cargo doc` |

Each workflow includes a summary gate job for branch protection compatibility.

## Building

```bash
cargo build --workspace
```

## License

MIT — see [LICENSE](LICENSE).
