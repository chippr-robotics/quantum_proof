# Comparison to Prior Work

## Oathbreaker vs. Google (March 2026)

| Aspect | Google | Oathbreaker |
|--------|--------|-------------|
| Curve | secp256k1 (256-bit) | Oath family (8/16/32/64-bit Goldilocks) |
| What's proven | Point addition only | Coherent group-action map [a]G + [b]Q |
| Coordinate system | Unknown (withheld) | Jacobian projective (single final inversion) |
| Classically verifiable | No (256-bit ECDLP infeasible) | Yes (Pollard's rho in hours) |
| Circuit published | No (withheld) | Yes (fully open-source) |
| Proof system | Custom ZK | SP1 Groth16 SNARK |
| Field arithmetic | Multi-limb (256-bit on 64-bit words) | Native (single 64-bit word) |
| Multiplier | Unknown | Karatsuba O(n^1.585) with symmetry-optimized squaring |
| Inverter | Unknown | Binary GCD (Kaliski) O(n^2) |
| Benchmark spec | No | Yes (Oathbreaker Scale) |
| Full Shor | Not claimed | Group-action core (v1); QFT deferred (v2) |
| Tests | Unknown | 40 tests + proptest CI |

## Resource Estimates (256-bit ECDLP)

| Author | Year | Qubits | Toffoli | Notes |
|--------|------|--------|---------|-------|
| Roetteler et al. | 2017 | 2,330 | 448B | First detailed estimates |
| Haner et al. | 2020 | 2,048 | 126B | Improved circuits |
| Litinski | 2023 | N/A | 50M | Measurement-based uncomputation |
| INRIA (Chevignard et al.) | 2026 | TBD | TBD | EUROCRYPT 2026 |
| Google (Babbush et al.) | 2026 | ≤1,450 | ≤90M | Low-gate variant; circuit withheld |
| **Oathbreaker (projected)** | **2026** | **~22,800** | **~1.2B** | **Oath-32 → 256 Karatsuba projection** |

## Measured Oathbreaker Results

### Per-Tier Circuit Measurements

| Tier | Qubits | Toffoli | Window | Multiplier |
|------|--------|---------|--------|------------|
| Oath-8 | 295 | 162K | w=4 | Karatsuba |
| Oath-16 | 855 | 997K | w=4 | Karatsuba |
| Oath-32 | 2,848 | 5.76M | w=8 | Karatsuba |

### Coordinate System Comparison (Oath-8)

| Coordinate System | Qubits | Toffoli | Inversions/add |
|-------------------|--------|---------|----------------|
| Affine + Fermat | lower | higher | ~128 total |
| **Jacobian + Binary GCD** | **higher** | **~25x fewer** | **1 total** |

The Jacobian circuit trades additional qubits for dramatically fewer Toffoli gates
by eliminating per-addition field inversions. The qubit increase comes from the
additional Z coordinate register and wider ancilla pool for Karatsuba multiplication.

### Cost Attribution (Oath-32, w=8)

| Subsystem | Toffoli | Share |
|-----------|---------|-------|
| Doublings | 4,541,952 | 80.2% |
| Mixed additions | 971,616 | 17.1% |
| Inversion (BGCD) | 107,008 | 1.9% |
| Affine recovery | 36,868 | 0.7% |
| QROM decode/load | 8,160 | 0.1% |

## Gap Analysis: Oathbreaker vs. Litinski (50M Toffoli)

Our projected 1.2B Toffoli is ~24x higher than Litinski's 50M. The gap is attributable to:

| Factor | Estimated Impact | Status |
|--------|-----------------|--------|
| **Measurement-based uncomputation** | ~2x | Not implemented (requires mid-circuit measurement) |
| **Semi-classical oracle** | ~2-4x | Litinski avoids coherent QROM entirely |
| **Windowed non-adjacent form (wNAF)** | ~1.2x | Not implemented |
| **Optimized doubling formulas** | ~1.3x | Doublings are 80% of cost; room for improvement |
| **Constant factor tuning** | ~1.5x | Industrial-grade optimization |

Closing the remaining gap requires hardware-dependent features (mid-circuit measurement)
and algorithmic improvements (wNAF, formula optimization) that are on the v2 roadmap.

## Key Differences

### vs. Roetteler/Haner
- We implement and measure the full group-action circuit, not just estimate
- Our Oath-8/16/32 measurements provide concrete anchors for scaling projections
- We include the dual-scalar formulation, not just single-scalar arithmetic
- We implement both affine and Jacobian coordinate systems for direct comparison
- We use Karatsuba multiplication (O(n^1.585)) vs schoolbook (O(n^2))

### vs. Litinski
- Litinski's 50M Toffoli uses measurement-based uncomputation (requires active error correction)
- Our circuit uses standard reversible uncomputation (works on any gate-model machine)
- Both are valid — they target different hardware models
- Our multiplier uses Karatsuba decomposition with symmetry-optimized squaring
- Our inverter uses Binary GCD (O(n^2)) instead of Fermat (O(n^2.585))
- Remaining gap is primarily from measurement-based uncomputation (~2x Bennett overhead) and Litinski's semi-classical oracle approach

### vs. Google
- Google proved correctness of point addition at 256 bits
- We prove the coherent group-action map [a]G + [b]Q on Oath-8/16/32 (measured)
- Our proof is independently verifiable via classical ECDLP solving
- Our circuit is fully transparent and open-source
- We provide both affine and Jacobian implementations for comparison
- We provide the Oathbreaker Scale benchmark specification
- Our comparison to Google's numbers is approximate — their circuit uses unknown optimizations

## Optimization History

| Optimization | Oath-32 Toffoli | 256-bit Projection | Change |
|-------------|-----------------|-------------------|--------|
| Baseline (schoolbook + Fermat) | 8.38M | ~4.3B | — |
| + Karatsuba multiplication | 7.37M | ~1.6B | -12% / -63% |
| + Symmetry-optimized squaring | 6.62M | ~1.4B | -10% / -13% |
| + Binary GCD inversion | 5.67M | ~1.2B | -14% / -14% |
| + Proper Cuccaro arithmetic | 5.76M | ~1.2B | +1.7% (correctness) |
