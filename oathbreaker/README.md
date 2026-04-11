# Oathbreaker: Full End-to-End Shor Circuit for ECDLP

**Project: Chippr Robotics LLC — Open Source Research**
**Status: Design Plan — April 2026**

---

## Overview

A complete implementation of Shor's algorithm for solving the Elliptic Curve
Discrete Logarithm Problem (ECDLP) on the **Oath-64** curve — a toy elliptic curve
over the Goldilocks field (p = 2^64 - 2^32 + 1).

This is a full end-to-end quantum circuit description — not a quantum execution —
that implements Shor's algorithm for ECDLP, proven correct via SP1 zkVM
(Groth16 SNARK), and independently verified against classically-computed ground
truth via Pollard's rho.

## What This Is

- A complete, transparent, open-source Shor circuit for ECDLP
- Classically verifiable: the same ECDLP can be solved via Pollard's rho
- Proven correct via SP1 Groth16 SNARK
- A ready-to-execute quantum benchmark for future hardware

## What This Is NOT

- Not a quantum computer execution (no hardware exists to run this at scale)
- Not an attack tool (64-bit curves have ~32-bit security, trivially breakable classically)
- Not a reproduction of Google's withheld secp256k1 circuits

## Why This Matters

1. **Extends Google's methodology**: Google proved only the point addition subroutine. This proves the full Shor circuit end-to-end.
2. **Classically verifiable**: Unlike Google's 256-bit proof, we can independently solve the same ECDLP via Pollard's rho and confirm the circuit produces the correct answer.
3. **Transparent**: Fully open-source circuit, unlike Google's opaque ZK-only disclosure.
4. **Native prover performance**: SP1 operates natively over the Goldilocks field — no multi-limb emulation overhead.
5. **Quantum benchmark candidate**: The circuit description is a ready-to-execute program for future quantum hardware.

## The Oathbreaker Scale

Score your quantum computer by which Oath curve it can crack:

| Level | Curve | Security | Classical Solve Time | Target |
|-------|-------|----------|---------------------|--------|
| Oath-8 | 8-bit | ~4-bit | Instant | Proof of concept |
| Oath-16 | 16-bit | ~8-bit | Instant | Near-term devices |
| Oath-32 | 32-bit | ~16-bit | Milliseconds | Medium-term milestone |
| **Oath-64** | **64-bit** | **~32-bit** | **Minutes (Pollard rho)** | **Full benchmark** |

## Architecture

```
oathbreaker/
├── crates/
│   ├── goldilocks-field/         # GF(p) arithmetic, p = 2^64 - 2^32 + 1
│   ├── ec-goldilocks/            # Elliptic curve over GF(p) + ECDLP solvers
│   ├── reversible-arithmetic/    # Reversible (quantum-compatible) gates
│   ├── shor-circuit/             # Full Shor's algorithm circuit assembly
│   ├── sp1-program/              # SP1 guest program (proven inside zkVM)
│   ├── sp1-host/                 # SP1 host: proof generation + verification
│   └── benchmark/                # Resource counting + scaling projections
├── sage/                         # SageMath Oath-64 curve generation scripts
├── proofs/                       # Generated proof artifacts
├── docs/                         # Technical paper and documentation
└── scripts/                      # Pipeline and export scripts
```

## Building

```bash
cargo build --workspace
```

## Testing

```bash
cargo test --workspace
```

## Full Pipeline

```bash
./scripts/full_pipeline.sh
```

## License

MIT — see [LICENSE](LICENSE).

## References

1. Babbush, Zalcman, Gidney et al., "Securing Elliptic Curve Cryptocurrencies against Quantum Vulnerabilities", arXiv:2603.28846 (March 2026)
2. Chevignard, Fouque, Schrottenloher, "Quantum circuits for ECDLP", EUROCRYPT 2026
3. Roetteler, Naehrig, Svore, Lauter, "Quantum Resource Estimates for Computing Elliptic Curve Discrete Logarithms", ASIACRYPT 2017
4. Litinski, "How to compute a 256-bit elliptic curve private key with only 50 million Toffoli gates", 2023
5. Cuccaro, Draper, Kutin, Moulton, "A new quantum ripple-carry addition circuit", arXiv:quant-ph/0410184
