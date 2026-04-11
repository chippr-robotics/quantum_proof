# The Oathbreaker Scale — Oath-N Benchmark Tiers

## Overview

The Oathbreaker Scale is a standardized quantum computing benchmark based on solving ECDLP instances of increasing difficulty. Each **Oath-N** tier defines an elliptic curve discrete logarithm problem at a specific field size, with a corresponding circuit and classically-verifiable answer.

Score your quantum computer by which Oath curve it can crack.

## Tier Definitions

| Tier | Field Size | Est. Logical Qubits | Est. Toffoli | Classical Difficulty | Target Hardware Era |
|------|-----------|---------------------|-------------|---------------------|-------------------|
| **Oath-8** | ~8 bit | ~20 | ~2,000 | Trivial (by hand) | 2026-2027 |
| **Oath-16** | ~16 bit | ~50 | ~15,000 | Trivial (milliseconds) | 2027-2028 |
| **Oath-32** | ~32 bit | ~120 | ~300,000 | Easy (~seconds) | 2029-2031 |
| **Oath-64** | 64 bit | ~300 | ~5,000,000 | Moderate (~hours Pollard rho) | 2032-2035 |

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

## Verification

Every Oath-N instance has a classically computable ground truth:
- **Oath-8, Oath-16**: Brute force in microseconds
- **Oath-32**: BSGS or Pollard's rho in seconds
- **Oath-64**: Pollard's rho in hours

The circuit is proven correct via SP1 Groth16 SNARK. Hardware teams submit their answer and it is checked against the oracle.

## Future Levels

| Tier | Field Size | Notes |
|------|-----------|-------|
| Oath-128 | 128 bit | Significant quantum resources required |
| Oath-256 | 256 bit | Equivalent to secp256k1 / P-256 |
| Oath-384 | 384 bit | P-384 difficulty |
| Oath-521 | 521 bit | P-521 (highest standard curve) |
