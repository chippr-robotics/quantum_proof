# Oathbreaker

## Open Reversible Circuit Framework for ECDLP over the Goldilocks Field
### With zk-Proven Execution Trace and Resource Benchmarking

**Project: Chippr Robotics LLC — Open Source Research**
**Status: Design Plan — April 2026**

---

## Overview

A reversible circuit framework capturing the arithmetic and coherent group-action
components required by Shor's algorithm for ECDLP, on the **Oath-64** curve — a
toy elliptic curve over the Goldilocks field (p = 2^64 - 2^32 + 1).

The circuit implements the coherent double-scalar map **[a]G + [b]Q** — the
computationally dominant component (>99% of qubits and gates) of Shor's ECDLP
algorithm. Proven correct via SP1 zkVM (Groth16 SNARK) and independently verified
against classical ground truth via Pollard's rho.

## What This Is

- A reversible circuit + classical simulator of quantum arithmetic subroutines
- The coherent group-action map [a]G + [b]Q, resource-counted and QASM-exportable
- Classically verifiable: the same ECDLP can be solved via Pollard's rho
- Proven correct via SP1 Groth16 SNARK (execution trace, not quantum simulation)
- A quantum hardware benchmark candidate (the Oathbreaker Scale)

## What This Is NOT

- Not a quantum computer execution (no hardware exists at 64-bit scale)
- Not an attack tool (64-bit curves have ~32-bit security, trivially breakable)
- Not a full Shor implementation in v1 (QFT + measurement deferred to v2)
- Not a reproduction of withheld industrial circuits, but an open analogue

## The Oathbreaker Scale

Score your quantum computer by which Oath curve it can crack:

| Tier | Field | Est. Qubits | Est. Toffoli | Classical Difficulty | Target Era |
|------|-------|-------------|-------------|---------------------|-----------|
| Oath-8 | 8-bit | ~20 | ~2K | Trivial | 2026-2027 |
| Oath-16 | 16-bit | ~50 | ~15K | Trivial | 2027-2028 |
| Oath-32 | 32-bit | ~120 | ~300K | Easy (~seconds) | 2029-2031 |
| **Oath-64** | **64-bit** | **~300** | **~5M** | **Hours (Pollard rho)** | **2032-2035** |

## Architecture

```
oathbreaker/
├── crates/
│   ├── goldilocks-field/           # GF(p) arithmetic, p = 2^64 - 2^32 + 1
│   ├── ec-goldilocks/              # EC ops + double-scalar mul + ECDLP solvers
│   ├── reversible-arithmetic/      # Reversible gates, adders, multipliers, EC ops
│   ├── group-action-circuit/       # Coherent [a]G + [b]Q circuit assembly
│   ├── sp1-program/                # SP1 guest program (proven inside zkVM)
│   ├── sp1-host/                   # SP1 host: proof generation + verification
│   └── benchmark/                  # Resource counting, Oath-N tiers, comparisons
├── sage/                           # SageMath Oath-64 curve generation scripts
├── proofs/                         # Generated proof artifacts
├── docs/                           # Paper, architecture, limitations, benchmarking
└── scripts/                        # Pipeline and export scripts
```

## v1 Scope

**Implements**: Reversible field arithmetic, reversible EC point add/double, coherent
double-scalar map [a]G + [b]Q, resource counting, ZK proof of execution trace.

**Defers to v2**: Dual-register QFT, quantum measurement simulation, classical
post-processing (lattice recovery of k from measured exponents).

See [docs/LIMITATIONS.md](docs/LIMITATIONS.md) for full scope details.

## Building

```bash
cargo build --workspace
```

## Testing

```bash
cargo test --workspace
```

## License

MIT — see [LICENSE](LICENSE).
