# Comparison to Prior Work

## Oathbreaker vs. Google (March 2026)

| Aspect | Google | Oathbreaker |
|--------|--------|-------------|
| Curve | secp256k1 (256-bit) | Oath-64 (64-bit Goldilocks) |
| What's proven | Point addition only | Coherent group-action map [a]G + [b]Q |
| Coordinate system | Unknown (withheld) | Jacobian projective (single final inversion) |
| Classically verifiable | No (256-bit ECDLP infeasible) | Yes (Pollard's rho in hours) |
| Circuit published | No (withheld) | Yes (fully open-source) |
| Proof system | Custom ZK | SP1 Groth16 SNARK |
| Field arithmetic | Multi-limb (256-bit on 64-bit words) | Native (single 64-bit word) |
| Benchmark spec | No | Yes (Oathbreaker Scale) |
| Full Shor | Not claimed | Group-action core (v1); QFT deferred (v2) |
| Tests | Unknown | 38 tests + proptest CI |

## Resource Estimates (256-bit ECDLP)

| Author | Year | Qubits | Toffoli | Notes |
|--------|------|--------|---------|-------|
| Roetteler et al. | 2017 | 2,330 | 448B | First detailed estimates |
| Haner et al. | 2020 | 2,048 | 126B | Improved circuits |
| Litinski | 2023 | N/A | 50M | Measurement-based uncomputation |
| INRIA (Chevignard et al.) | 2026 | TBD | TBD | EUROCRYPT 2026 |
| Google (Babbush et al.) | 2026 | N/A | N/A | Withheld |
| Oathbreaker (projected) | 2026 | TBD | TBD | Oath-64 -> 256 scaling |

## Oathbreaker Oath-64 Measured Estimates

| Coordinate System | Qubits | Toffoli | Inversions/add | Notes |
|-------------------|--------|---------|----------------|-------|
| Affine + Fermat | ~300 | ~5M | ~128 total | ~102 mul-equivalents per add |
| **Jacobian + 1 inv** | **~700** | **~17M** | **1 total** | ~16 mul per add, +20% qubits |

The Jacobian circuit trades ~2.3x more qubits for ~6x fewer mul-equivalents per
point addition. The qubit increase comes from the additional Z coordinate register
(64 qubits) and wider ancilla pool for the more complex addition formulas.

## Key Differences

### vs. Roetteler/Haner
- We implement and measure the full group-action circuit, not just estimate
- Our Oath-64 measurements provide a concrete anchor for scaling projections
- We include the dual-scalar formulation, not just single-scalar arithmetic
- We implement both affine and Jacobian coordinate systems for direct comparison

### vs. Litinski
- Litinski's 50M Toffoli uses measurement-based uncomputation (requires active error correction)
- Our circuit uses standard reversible uncomputation (works on any gate-model machine)
- Both are valid — they target different hardware models
- Our multiplier now uses Karatsuba decomposition (O(n^1.585) per multiply), closing part of the gap
- Remaining gap is primarily from measurement-based uncomputation (2× Bennett overhead) and Litinski's semi-classical oracle approach

### vs. Google
- Google proved correctness of point addition at 256 bits
- We prove the coherent group-action map [a]G + [b]Q on Oath-64
- Our proof is independently verifiable via classical ECDLP solving
- Our circuit is fully transparent and open-source
- We provide both affine and Jacobian implementations for comparison
- We provide the Oathbreaker Scale benchmark specification
- Our comparison to Google's numbers is approximate — their circuit uses unknown optimizations
