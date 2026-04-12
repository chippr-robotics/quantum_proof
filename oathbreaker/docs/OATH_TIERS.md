# The Oathbreaker Scale — Oath-N Benchmark Tiers

## Overview

The Oathbreaker Scale is a standardized quantum computing benchmark based on solving ECDLP instances of increasing difficulty. Each **Oath-N** tier defines an elliptic curve discrete logarithm problem at a specific field size, with a corresponding circuit and classically-verifiable answer.

Score your quantum computer by which Oath curve it can crack.

## Tier Definitions

Resource counts from measured circuit construction using Jacobian projective
coordinates with Karatsuba multiplication (O(n^1.585) per multiply),
symmetry-optimized squaring, Binary GCD inversion (O(n^2)), and windowed
scalar multiplication with one-hot QROM decode.

| Tier | Field Size | Measured Qubits | Measured Toffoli | Classical Difficulty | Target Hardware Era |
|------|-----------|-----------------|------------------|---------------------|-------------------|
| **Oath-8** | ~8 bit | 295 | 162,000 | Trivial (by hand) | 2026-2027 |
| **Oath-16** | ~16 bit | 855 | 997,000 | Trivial (milliseconds) | 2027-2028 |
| **Oath-32** | ~32 bit | 2,848 | 5,760,000 | Easy (~seconds) | 2029-2031 |
| **Oath-64** | 64 bit | ~5,696 (proj.) | ~90M (proj.) | Moderate (~hours Pollard rho) | 2032-2035 |

Oath-8/16/32 are measured from actual circuit construction. Oath-64 is projected
(circuit materialization exceeds CI memory at ~3 GB / ~90M gate objects).

### Cost Attribution (Oath-32, w=8)

| Subsystem | Toffoli | Share |
|-----------|---------|-------|
| Doublings | 4,541,952 | 80.2% |
| Mixed additions | 971,616 | 17.1% |
| Inversion (BGCD) | 107,008 | 1.9% |
| Affine recovery | 36,868 | 0.7% |
| QROM decode/load | 8,160 | 0.1% |

### Window Size Optimization

w=8 is confirmed optimal for Toffoli across all tiers:

| Tier | w=1 | w=2 | w=4 | w=8 |
|------|-----|-----|-----|-----|
| Oath-32 | 13.4M | 9.5M | 7.6M | **5.76M** |

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

| Tier | Field Size | Projected Toffoli (Karatsuba) | Notes |
|------|-----------|------------------------------|-------|
| Oath-128 | 128 bit | ~208M | Significant quantum resources required |
| Oath-256 | 256 bit | ~1.2B | Equivalent to secp256k1 / P-256 |
| Oath-384 | 384 bit | ~3.7B | P-384 difficulty |
| Oath-521 | 521 bit | ~9.3B | P-521 (highest standard curve) |
