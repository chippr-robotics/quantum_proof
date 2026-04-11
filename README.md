# Quantum Circuit Verification & the Oathbreaker Framework

**[Read the blog post](https://chippr-robotics.github.io/quantum_proof/)**

This repository contains two complementary projects:

1. **Educational explainer** of Google Quantum AI's March 2026 zero-knowledge proof for ECDLP circuit resource estimates
2. **Oathbreaker** — a fully open-source reversible circuit framework implementing the coherent group-action map for Shor's ECDLP algorithm over the Goldilocks field, with Jacobian projective coordinates, classical verification, and a standardized quantum hardware benchmark (the Oathbreaker Scale)

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
├── cryptocurrency-whitepaper.pdf                # Source paper (Google Quantum AI)
├── oathbreaker/                                # Reversible circuit framework
│   ├── crates/
│   │   ├── goldilocks-field/                   # GF(p) arithmetic, p = 2^64 - 2^32 + 1
│   │   ├── ec-goldilocks/                      # EC ops, Jacobian + affine, ECDLP solvers
│   │   ├── reversible-arithmetic/              # Reversible gates, adders, multipliers, EC ops
│   │   ├── group-action-circuit/               # Coherent [a]G + [b]Q circuit assembly
│   │   ├── benchmark/                          # Resource counting, Oath-N tiers, comparisons
│   │   ├── sp1-program/                        # SP1 guest program (proven inside zkVM)
│   │   └── sp1-host/                           # SP1 host: proof generation + verification
│   ├── sage/                                   # SageMath curve generation scripts
│   ├── proofs/                                 # Generated proof artifacts
│   ├── docs/                                   # Architecture, benchmarking, limitations
│   └── README.md                               # Oathbreaker project documentation
├── .github/workflows/                          # CI: unit tests, functional tests, code quality
└── README.md                                   # This file
```

### Oathbreaker Framework (`oathbreaker/`)

A fully implemented reversible circuit framework for Shor's ECDLP algorithm on the **Oath-64** curve (Goldilocks field, p = 2^64 - 2^32 + 1). The framework implements every layer of the circuit stack:

- **Goldilocks field arithmetic** — addition, multiplication, inversion, square root (Tonelli-Shanks), with property-based testing of all field axioms
- **Elliptic curve operations** — point addition, doubling, and scalar multiplication in both affine and Jacobian projective coordinates
- **Reversible circuit primitives** — NOT/CNOT/Toffoli gates, Cuccaro ripple-carry adder, schoolbook multiplier, Fermat and binary GCD inverters
- **Reversible EC operations** — point addition and doubling in both affine and Jacobian coordinates, with full intermediate uncomputation
- **Circuit assembly** — windowed scalar multiplication, precomputed lookup tables, coherent double-scalar map [a]G + [b]Q
- **Classical verification** — Pollard's rho ECDLP solver for independent ground-truth checking

38 tests pass across 4 core crates, including 7 property-based tests (proptest) that stress-test algebraic invariants with 1024 random cases each in CI.

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

# Run all tests (38 tests across 4 core crates)
cargo test --workspace

# Run the benchmark suite
cargo run --release -p benchmark
```

## Continuous Integration

Three GitHub Actions workflows enforce correctness at every layer:

| Workflow | What it checks |
|----------|---------------|
| **Unit Tests** | Per-crate isolation testing + property-based tests (1024 cases/property) |
| **Functional Tests** | Workspace integration, dependency-chain validation, benchmark execution, SP1 compilation |
| **Code Quality** | `rustfmt` formatting, `clippy` linting (strict on core crates), `cargo doc` build |

## Key Concepts

- **ECDLP**: The Elliptic Curve Discrete Logarithm Problem — given points G and Q = kG, find k. This is the cryptographic foundation of Bitcoin, Ethereum, and most major blockchains.
- **Shor's Algorithm**: A quantum algorithm that solves ECDLP in polynomial time, rendering ECDLP-based cryptography insecure against quantum computers.
- **Kickmix Circuits**: Classically-simulable quantum circuits composed of reversible logic gates and measurement-based uncomputation. They can be verified inside a classical virtual machine.
- **Fiat-Shamir Heuristic**: A technique that derives challenge values from the prover's own commitment, converting an interactive proof into a non-interactive one.
- **SP1 zkVM**: A zero-knowledge virtual machine that executes RISC-V programs and generates cryptographic proofs of correct execution.
- **Groth16 SNARK**: A succinct non-interactive argument of knowledge that provides zero-knowledge and fast verification.
- **Jacobian Projective Coordinates**: A coordinate system for elliptic curves that eliminates per-operation modular inversions, reducing gate cost by ~6x compared to affine coordinates.
- **The Oathbreaker Scale**: A standardized quantum hardware benchmark — score a quantum computer by which Oath-N curve it can crack.

## References

- Babbush, Zalcman, Gidney et al. "Securing Elliptic Curve Cryptocurrencies against Quantum Vulnerabilities" (2026) — [PDF](https://quantumai.google/static/site-assets/downloads/cryptocurrency-whitepaper.pdf)
- Litinski, D. "How to compute a 256-bit elliptic curve private key with only 50 million Toffoli gates" (2023)
- Groth, J. "On the Size of Pairing-based Non-interactive Arguments" (2016)
- [SP1 zkVM — Succinct Labs](https://github.com/succinctlabs/sp1)

## Disclaimer

This repository is an independent educational resource. It is not affiliated with or endorsed by Google Quantum AI. The content is based on the publicly available whitepaper linked above.
