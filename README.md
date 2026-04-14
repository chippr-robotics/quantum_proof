# Quantum Circuit Verification & the Oathbreaker Framework

**[Read the blog post](https://chippr-robotics.github.io/quantum_proof/)**

This repository contains two complementary projects:

1. **Educational explainer** of Google Quantum AI's March 2026 zero-knowledge proof for ECDLP circuit resource estimates
2. **Oathbreaker** — a fully open-source reversible circuit framework implementing Shor's ECDLP algorithm over Goldilocks-form fields, with Jacobian projective coordinates, Karatsuba multiplication, Binary GCD inversion, QFT, measurement, classical post-processing, and a standardized quantum hardware benchmark (the Oathbreaker Scale)

## Background

In March 2026, Google Quantum AI published [*"Securing Elliptic Curve Cryptocurrencies against Quantum Vulnerabilities"*](https://quantumai.google/static/site-assets/downloads/cryptocurrency-whitepaper.pdf), demonstrating that Shor's algorithm for the 256-bit Elliptic Curve Discrete Logarithm Problem (ECDLP) over the secp256k1 curve can execute with:

| Variant | Logical Qubits | Toffoli Gates |
|---------|---------------|---------------|
| Low-Qubit | ≤ 1,200 | ≤ 90 million |
| Low-Gate | ≤ 1,450 | ≤ 70 million |

On superconducting hardware with 10⁻³ error rates, these circuits could run in under 25 minutes using fewer than 500,000 physical qubits — roughly a 20x reduction over prior estimates.

To substantiate these claims without disclosing the attack circuits, the authors produced a **Groth16 zero-knowledge SNARK** (via the SP1 zkVM) proving they possess circuits that correctly compute secp256k1 point addition within the claimed resource bounds.

## What's in This Repository

```
quantum_proof/
├── index.html                                  # GitHub Pages blog post
├── notebook/
│   └── quantum_verification_walkthrough.ipynb   # Interactive Jupyter walkthrough
├── oathbreaker/                                # Reversible circuit framework
│   ├── crates/
│   │   ├── goldilocks-field/                   # GF(p) arithmetic, p = 2^64 - 2^32 + 1
│   │   ├── ec-goldilocks/                      # EC ops, Jacobian + affine, ECDLP solvers
│   │   ├── reversible-arithmetic/              # Reversible gates, Karatsuba multiplier, Binary GCD
│   │   ├── group-action-circuit/               # Coherent [a]G + [b]Q circuit assembly
│   │   ├── benchmark/                          # Resource counting, cost attribution, scaling
│   │   ├── sp1-program/                        # SP1 guest program (circuit verification inside zkVM)
│   │   └── sp1-host/                           # SP1 host: classical verification + Groth16 proof generation
│   ├── sage/                                   # SageMath curve generation + verification scripts
│   ├── proofs/                                 # Generated proof artifacts
│   ├── docs/                                   # Architecture, benchmarking, paper draft
│   │   └── paper/                              # LaTeX technical paper
│   └── README.md                               # Oathbreaker project documentation
├── .github/workflows/                          # CI: tests, benchmarks, curve verification
└── README.md                                   # This file
```

### Oathbreaker Framework (`oathbreaker/`)

A complete implementation of Shor's ECDLP algorithm on the **Oath curve family** (Goldilocks-form prime fields). The framework implements every layer of the circuit stack:

- **Goldilocks field arithmetic** — addition, multiplication, inversion, square root (Tonelli-Shanks), with property-based testing of all field axioms
- **Elliptic curve operations** — point addition, doubling, and scalar multiplication in both affine and Jacobian projective coordinates
- **Reversible circuit primitives** — NOT/CNOT/Toffoli gates, Cuccaro ripple-carry adder, Karatsuba multiplier (O(n^1.585)), symmetry-optimized squarer, Binary GCD and Fermat inverters
- **Reversible EC operations** — point addition and doubling in both affine and Jacobian coordinates, with proper Cuccaro subtraction for all field operations
- **Circuit assembly** — windowed scalar multiplication with one-hot QROM decode, precomputed lookup tables, coherent double-scalar map [a]G + [b]Q
- **Quantum Fourier Transform** — full forward/inverse QFT gate generation, classical DFT simulation verified gate-by-gate, QASM export with Hadamard, controlled-phase, and SWAP gates
- **Measurement + classical recovery** — Shor measurement simulation, continued fraction expansion, direct modular inversion, and multi-measurement lattice recovery of the discrete log
- **Benchmarking** — v3 measured resource counts for Oath-8/16/32, per-subsystem cost attribution, window-size sweep, three-model scaling projections to 256-bit
- **Classical verification** — Pollard's rho ECDLP solver for independent ground-truth checking
- **ZK proof** — SP1 guest/host programs with feature-gated Groth16 SNARK generation, classical verification mode for CI, following Google's zkp_ecc architecture
- **Curve verification** — SageMath SEA verification of all Oath-N parameters in CI

**Current results (Oath-32, v3 measured)**: 1,058 qubits, 5.64M Toffoli gates.
**256-bit projection**: ~1.2B Toffoli (Karatsuba model), ~24x gap to Litinski's 50M.

112 tests pass across 4 core crates, including 7 property-based tests (proptest) that stress-test algebraic invariants with 1024 random cases each in CI.

See [oathbreaker/README.md](oathbreaker/README.md) for full details.

### Blog Post (`index.html`)

A self-contained web page (designed for GitHub Pages) that explains:
- What the ZK proof demonstrates and why it's needed
- The verification pipeline: SHA-256 commitment → Fiat-Shamir fuzz testing → SP1 zkVM → Groth16 SNARK
- How point addition costs scale to full ECDLP circuit costs
- Physical resource estimates and attack timing analysis

**View it live:** Enable GitHub Pages on this repository (Settings → Pages → Deploy from branch) and visit the published URL.

### Jupyter Notebook (`notebook/quantum_verification_walkthrough.ipynb`)

A step-by-step interactive walkthrough covering:
1. **secp256k1 elliptic curve arithmetic** — point addition and scalar multiplication
2. **The ECDLP** — why it matters for cryptocurrency security
3. **Fiat-Shamir fuzz testing** — soundness analysis and why 9,024 tests give 128-bit security
4. **Resource cost derivation** — from point addition costs to full ECDLP circuit costs
5. **Physical resource estimation** — mapping logical circuits to superconducting hardware
6. **On-spend attack probability** — modeling the race between attacker and blockchain confirmation
7. **ZK proof pipeline summary** — the complete verification flow

#### Running the Notebook

```bash
pip install jupyter numpy matplotlib
jupyter notebook notebook/quantum_verification_walkthrough.ipynb
```

## Building & Testing

```bash
# Build the oathbreaker workspace
cd oathbreaker && cargo build --workspace

# Run all tests (91 tests across 4 core crates)
cargo test --workspace

# Run the benchmark suite (resource counting + scaling projections)
cargo run --release -p benchmark

# Export OpenQASM 3.0 circuit
cargo run --release -p benchmark -- export-qasm
```

## Continuous Integration

Five GitHub Actions workflows enforce correctness at every layer:

| Workflow | What it checks |
|----------|---------------|
| **Unit Tests** | Per-crate isolation testing + property-based tests (1024 cases/property) |
| **Functional Tests** | Workspace integration, dependency-chain validation, benchmark execution, SP1 compilation |
| **Code Quality** | `rustfmt` formatting, `clippy` linting (strict on core crates), `cargo doc` build |
| **Benchmark** | Full resource counting suite, QASM export, scaling projections, PR job summary |
| **Curve Verification** | SageMath SEA verification of all Oath-N parameters, SHA-256 audit trail |

## Key Concepts

- **ECDLP**: The Elliptic Curve Discrete Logarithm Problem — given points G and Q = kG, find k. This is the cryptographic foundation of Bitcoin, Ethereum, and most major blockchains.
- **Shor's Algorithm**: A quantum algorithm that solves ECDLP in polynomial time, rendering ECDLP-based cryptography insecure against quantum computers.
- **Kickmix Circuits**: Classically-simulable quantum circuits composed of reversible logic gates and measurement-based uncomputation. They can be verified inside a classical virtual machine.
- **Fiat-Shamir Heuristic**: A technique that derives challenge values from the prover's own commitment, converting an interactive proof into a non-interactive one.
- **SP1 zkVM**: A zero-knowledge virtual machine that executes RISC-V programs and generates cryptographic proofs of correct execution.
- **Groth16 SNARK**: A succinct non-interactive argument of knowledge that provides zero-knowledge and fast verification.
- **Jacobian Projective Coordinates**: A coordinate system for elliptic curves that eliminates per-operation modular inversions, reducing gate cost by ~25x compared to affine coordinates.
- **Karatsuba Multiplication**: A divide-and-conquer algorithm achieving O(n^1.585) gate cost per multiplication, replacing O(n^2) schoolbook multiplication.
- **Binary GCD Inversion**: An O(n^2) algorithm for modular inversion, replacing Fermat's O(n^2.585) method. Based on Kaliski's variant of the extended GCD.
- **The Oathbreaker Scale**: A standardized quantum hardware benchmark — score a quantum computer by which Oath-N curve it can crack.

## References

- Babbush, Zalcman, Gidney et al. "Securing Elliptic Curve Cryptocurrencies against Quantum Vulnerabilities" (2026) — [PDF](https://quantumai.google/static/site-assets/downloads/cryptocurrency-whitepaper.pdf)
- Litinski, D. "How to compute a 256-bit elliptic curve private key with only 50 million Toffoli gates" (2023)
- Roetteler, M. et al. "Quantum resource estimates for computing elliptic curve discrete logarithms" (ASIACRYPT 2017)
- Groth, J. "On the Size of Pairing-based Non-interactive Arguments" (2016)
- Cuccaro, S. et al. "A new quantum ripple-carry addition circuit" (2004)
- [SP1 zkVM — Succinct Labs](https://github.com/succinctlabs/sp1)

## Disclaimer

This repository is an independent educational resource. It is not affiliated with or endorsed by Google Quantum AI. The content is based on the publicly available whitepaper linked above.
