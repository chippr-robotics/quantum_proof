# The Oathbreaker Scale — Oath-N Benchmark Tiers

## Overview

The Oathbreaker Scale is a standardized quantum computing benchmark based on solving ECDLP instances of increasing difficulty. Each **Oath-N** tier defines an elliptic curve discrete logarithm problem at a specific field size, with a corresponding circuit and classically-verifiable answer.

Score your quantum computer by which Oath curve it can crack.

## Tier Definitions

Resource counts from measured circuit construction using Jacobian projective
coordinates with Karatsuba multiplication (O(n^1.585) per multiply),
symmetry-optimized squaring, Binary GCD inversion (O(n^2)), and windowed
scalar multiplication with one-hot QROM decode.

| Tier | Field Size | Qubits (Bennett) | Qubits (meas-based est.) | Toffoli | Classical Difficulty | Target Hardware Era |
|------|-----------|-------------------|--------------------------|---------|---------------------|-------------------|
| **Oath-8** | ~8 bit | 218 | 186 | 114,000 | Trivial (by hand) | 2026-2027 |
| **Oath-16** | ~16 bit | 418 | 370 | 934,000 | Trivial (milliseconds) | 2027-2028 |
| **Oath-32** | ~32 bit | 1,058 | 738 | 5,641,000 | Easy (~seconds) | 2029-2031 |
| **Oath-64** | 64 bit | ~2,116 (proj.) | ~1,476 (proj.) | ~34M (proj.) | Moderate (~hours Pollard rho) | 2032-2035 |

Oath-8/16/32 are measured from actual circuit construction with proper ancilla
reuse between phases. Oath-64 is projected (circuit materialization exceeds CI
memory at ~3 GB). "Qubits (Bennett)" is the peak with standard reversible
uncomputation. "Qubits (meas-based est.)" estimates the reduction achievable
with mid-circuit measurement and classical feedforward.

### Cost Attribution (Oath-32, v3, w=8)

| Subsystem | Toffoli | Share |
|-----------|---------|-------|
| Doublings | 4,408,320 | 78.1% |
| Mixed additions | 1,080,736 | 19.2% |
| Inversion (BGCD) | 107,008 | 1.9% |
| Affine recovery | 36,868 | 0.7% |
| QROM decode/load | 8,160 | 0.1% |

### Window Size Optimization

w=8 is confirmed optimal for Toffoli across all tiers:

| Tier | w=1 | w=2 | w=4 | w=8 |
|------|-----|-----|-----|-----|
| Oath-32 | 13.2M | 8.9M | 6.7M | **5.64M** |

## Scoring Rules

### Input
Each Oath-N instance provides:
1. Curve parameters (a, b, p, generator G)
2. Target point Q = [k]G
3. The QASM circuit description

### Expected Output
The discrete logarithm k such that Q = [k]G.

### Scoring
A quantum hardware execution is scored by:
- **Correctness**: Did it recover the right k? (Verified against classical oracle)
- **Shots**: How many executions were needed?
- **Wall time**: Total time from first shot to correct answer
- **Physical qubits**: Including error correction overhead
- **Error rate**: Observed rate of incorrect outputs

### Grading
- **Pass/fail**. The correct k is verified against the classical oracle.
- **No partial credit.** Either the machine produced the right discrete log or it didn't.
- A machine's **Oathbreaker rating** is the highest Oath-N level it has passed.

## Curve Generation

Each Oath-N curve is generated via SageMath with the following properties:
- Prime group order (no cofactor)
- Non-anomalous (order != field characteristic)
- Embedding degree > 4
- Generator of full order verified

For Oath-64, the field is the Goldilocks prime p = 2^64 - 2^32 + 1.
Smaller tiers use appropriate primes near the target bit size.

All parameters are independently verified in CI via SageMath's SEA
(Schoof-Elkies-Atkin) algorithm, completely independent of the generation
script. SHA-256 fingerprints of parameter files are recorded in the
GitHub Actions job summary for each commit.

## Verification

Every Oath-N instance has a classically computable ground truth:
- **Oath-8, Oath-16**: Brute force in microseconds
- **Oath-32**: BSGS or Pollard's rho in seconds
- **Oath-64**: Pollard's rho in hours

The circuit is proven correct via SP1 Groth16 SNARK. Hardware teams submit their answer and it is checked against the oracle.

## QASM Export

```bash
# Export single tier (default: Oath-16)
cargo run --release -p benchmark -- export-qasm

# Export all tiers
cargo run --release -p benchmark -- export-all-qasm
```

Produces OpenQASM 3.0 files for each Oath level, loadable in Qiskit, Cirq, and other frameworks.

## Future Levels

| Tier | Field Size | Projected Qubits (Bennett) | Projected Qubits (meas-based) | Projected Toffoli | Notes |
|------|-----------|---------------------------|-------------------------------|-------------------|-------|
| Oath-128 | 128 bit | ~4,232 | ~2,952 | ~203M | Significant quantum resources required |
| Oath-256 | 256 bit | ~8,464 | ~5,904 | ~1.2B | Equivalent to secp256k1 / P-256 |
| Oath-384 | 384 bit | ~12,696 | ~8,856 | ~3.5B | P-384 difficulty |
| Oath-521 | 521 bit | ~17,226 | ~12,016 | ~7.6B | P-521 (highest standard curve) |
