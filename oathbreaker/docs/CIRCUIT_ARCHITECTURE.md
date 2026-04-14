# Circuit Architecture

## Overview

The Oathbreaker circuit implements the **coherent double-scalar group-action map**:

```
|a⟩|b⟩|O⟩ → |a⟩|b⟩|[a]G + [b]Q⟩
```

This is the computationally dominant component (>99% of qubits and gates) of Shor's
ECDLP algorithm. Every classical computation is decomposed into NOT, CNOT, and
Toffoli gates operating on a qubit register model.

The circuit targets the **Oath curve family** over Goldilocks-form prime fields, with
measured results at 8, 16, and 32 bits and projections to 256+ bits.

## Register Layout (Jacobian Projective)

For an n-bit field:

```
┌──────────────────────┐
│ Exponent A Register  │  n qubits  (scalar for [a]G)
│ [a_0, a_1, ..., a_n] │
├──────────────────────┤
│ Exponent B Register  │  n qubits  (scalar for [b]Q)
│ [b_0, b_1, ..., b_n] │
├──────────────────────┤
│ Point X Register     │  n qubits  (Jacobian X coordinate)
│ [X_0, X_1, ..., X_n] │
├──────────────────────┤
│ Point Y Register     │  n qubits  (Jacobian Y coordinate)
│ [Y_0, Y_1, ..., Y_n] │
├──────────────────────┤
│ Point Z Register     │  n qubits  (Jacobian Z coordinate)
│ [Z_0, Z_1, ..., Z_n] │
├──────────────────────┤
│ One-Hot Register     │  2^w qubits (QROM selection)
├──────────────────────┤
│ Ancilla Pool         │  ~N qubits  (Karatsuba workspace + EC intermediates)
│ [anc_0, ..., anc_N]  │
└──────────────────────┘
```

**Note**: Jacobian adds one n-bit Z register (+n qubits) vs affine.
The tradeoff: +qubits, dramatically fewer Toffoli (elimination of per-op inversions).

**Measured qubit counts (v3)** (including all ancillae):

| Tier | Primary (6n) | One-Hot (2^w) | Ancillae | Total |
|------|-------------|--------------|----------|-------|
| Oath-8 | 48 | 16 | ~154 | 218 |
| Oath-16 | 96 | 16 | ~306 | 418 |
| Oath-32 | 192 | 256 | ~610 | 1,058 |

Note: v3 uses 6n primary qubits (vs 5n in v2) due to the cached aZ⁴ register
in modified Jacobian coordinates, but achieves lower total qubit count through
tighter ancilla management (12n+2 doubler workspace vs 14n+2 in v2).

## Group-Action Circuit Flow

```
1. Controlled [a]G computation:
   For each window of exponent register A:
     - w doublings of accumulator (Jacobian, 6S+3M each)
     - One-hot decode of w scalar bits → 2^w selection register
     - CNOT-load selected precomputed table entry
     - Jacobian mixed addition: accumulator += table entry (3S+8M)
     - Reverse one-hot decode (clean selection register)
     - Uncompute QROM ancillae

2. Controlled [b]Q addition:
   For each window of exponent register B:
     - Same as above with Q's precomputed table

3. Final inversion:
   - Binary GCD (Kaliski) inverter: Z → Z⁻¹ (O(n²) Toffoli)

4. Affine recovery:
   - x = X·Z⁻², y = Y·Z⁻³ (2 Karatsuba multiplications)

5. Inverse QFT on both exponent registers
   - Hadamard + controlled-phase rotations + SWAP (bit reversal)
   - Applied independently to register A and register B

6. Measurement of both exponent registers → (c, d)

7. Classical post-processing: k = -c · d⁻¹ (mod order)
```

## Gate Decomposition: Multiplication

### Karatsuba Multiplier (Primary — O(n^1.585))

The Karatsuba multiplier recursively splits n-bit operands into halves,
performing 3 sub-multiplications instead of 4:

```
a × b:
  Split: a = a_hi·2^(n/2) + a_lo,  b = b_hi·2^(n/2) + b_lo
  z0 = a_lo × b_lo
  z2 = a_hi × b_hi
  z1 = (a_lo + a_hi)(b_lo + b_hi) - z0 - z2     (Cuccaro subtraction)
  result = z0 + z1·2^(n/2) + z2·2^n              (shift + add)
  → Goldilocks modular reduction
  → CNOT copy to output
  → Bennett uncomputation (reverse all forward gates)
```

Base case at n ≤ 8: falls back to schoolbook partial products.

### Symmetry-Optimized Squaring

For a², cross-term symmetry eliminates ~50% of partial-product Toffoli:
- Each a[i]·a[j] = a[j]·a[i] pair is computed once (placed at position i+j+1)
- Diagonal terms a[i]² = a[i] use CNOT instead of Toffoli
- The `is_square` flag propagates through Karatsuba recursion

### Cuccaro Subtraction

Proper reversible subtraction using Cuccaro adder gates in reverse order.
All field subtractions (H = U2 - X1, R = S2 - Y1, X3 = R² - H³ - 2·X1·H², etc.)
use `cuccaro_subtract()` for correct arithmetic (not XOR).

## Gate Decomposition: Point Operations

### Jacobian Mixed Addition (3S + 8M = 11 field operations)

The accumulator is in Jacobian projective coordinates (X, Y, Z). Table entries
are in affine (x, y). Mixed addition avoids **all per-addition inversions**:

1. **Squaring** (Z₁²): Karatsuba squarer
2. **Multiplication** (Z₁³ = Z₁² · Z₁): Karatsuba multiplier
3. **Multiplication** (U₂ = x₂ · Z₁²): Karatsuba multiplier
4. **Multiplication** (S₂ = y₂ · Z₁³): Karatsuba multiplier
5. **Subtraction** (H = U₂ - X₁): Cuccaro subtract, O(n)
6. **Subtraction** (R = S₂ - Y₁): Cuccaro subtract, O(n)
7. **Squaring** (H²): Karatsuba squarer
8. **Multiplication** (H³ = H² · H): Karatsuba multiplier
9. **Multiplication** (X₁·H²): Karatsuba multiplier
10. **Squaring** (R²): Karatsuba squarer
11. **Subtractions + multiplications** (X₃, Y₃, Z₃): Cuccaro subtract + Karatsuba

**Total: 3 squarings + 8 multiplications, 0 inversions.**

### Jacobian Point Doubling

**v3 (Modified Jacobian, 4S + 4M = 8 field operations)**:

Uses modified Jacobian coordinates (X, Y, Z, aZ⁴), caching aZ⁴ to eliminate
two squarings per doubling:

1. **Squaring** (Y₁²) → A: Karatsuba squarer
2. **Multiplication** (X₁ · A) → S: Karatsuba multiplier
3. **Constant multiply** (4·S) → B: Cuccaro additions
4. **Squaring** (A²): Karatsuba squarer
5. **Constant multiply** (8·A²) → C: Cuccaro additions
6. **Compute D** (3·X₁² + aZ₁⁴): uses cached aZ⁴ directly
7. **Squaring** (D²): Karatsuba squarer
8. **Subtraction** (X₃ = D² - 2·B): Cuccaro subtract
9. **Multiplication** (Y₃ = D·(B - X₃) - C): Karatsuba multiplier
10. **Multiplication** (Z₃ = 2·Y₁·Z₁): Karatsuba multiplier
11. **Multiplication** (aZ₃⁴ update = 2·T·aZ₁⁴): Karatsuba multiplier

**Workspace**: 12n + 2 qubits (-14% vs v2's 14n + 2).

**v2 (Standard Jacobian, 6S + 3M = 9 field operations)**: Computed Z₁² and Z₁⁴
from scratch each doubling. See V3_OPTIMIZATIONS.md for the formula comparison.

### Binary GCD Inversion (O(n²) Toffoli)

Replaces Fermat's method (125 multiplications, O(n^2.585)) with an O(n²) algorithm
using only additions, subtractions, and shifts. Based on Roetteler et al.
(ASIACRYPT 2017, Section 4).

**Phase 1 — Extended Binary GCD (2n iterations)**:
- State: (u, v, r, s) initialized to (p, a, 0, 1)
- Each iteration: conditional swap, optional subtract, right-shift u, left-shift s
- Cost: ~20n Toffoli per iteration

**Phase 2 — Montgomery Correction (2n halvings)**:
- Convert "almost inverse" a⁻¹·2^k to true a⁻¹ mod p
- Each iteration: conditional add p, right-shift r
- Cost: ~8n Toffoli per iteration

**Total**: ~96n² Toffoli (including Bennett uncomputation).

**Improvement at Oath-32**: 960K → 107K Toffoli (-89%), reducing inversion's
share from ~15% to 1.9% of total circuit cost.

## QROM One-Hot Decode

The QROM (Quantum Read-Only Memory) loads a classically precomputed
table entry into quantum registers, controlled by w scalar bits.

1. Convert w scalar bits into 2^w one-hot selection register via binary decode
   (O(2^w) Toffoli — each bit splits active entries into bit=0/bit=1 branches)
2. CNOT-load the selected table entry's (x, y) coordinates (O(2^w × n) CNOT)
3. Reverse the one-hot decode to clean the selection register

Total QROM cost: O(2^w) Toffoli + O(2^w × n) CNOT per window.
At w=8: 256 Toffoli per decode, <0.1% of total circuit cost.

## Ancilla Strategies

### Eager Uncomputation (Bennett's Compute-Copy-Uncompute)
- Every multiplication/squaring/inversion uses this pattern
- Forward gates collected → result CNOT-copied to output → gates replayed in reverse
- Cleans all workspace qubits back to |0⟩
- ~2x gate overhead vs measurement-based approaches
- **Currently implemented**

### Measurement-Based Uncomputation (Not Implemented)
- Measure ancilla qubits instead of reversing
- Requires mid-circuit measurement + classical feedforward
- Would roughly halve total gate count
- Hardware-dependent; deferred to future version

## Windowed Scalar Multiplication

Applied independently to both scalar registers:

| Window Size | Iterations (n=32) | Table Size | Toffoli (Oath-32, v3) | Qubits |
|-------------|-------------------|------------|----------------------|--------|
| 1 | 32 | 2 points | 13.2M | 804 |
| 2 | 16 | 4 points | 8.9M | 806 |
| 4 | 8 | 16 points | 6.7M | 818 |
| **8** | **4** | **256 points** | **5.64M** | **1,058** |

w=8 is monotonically best for Toffoli across all measured tiers.

## QFT Component

The inverse QFT is applied independently to both n-qubit exponent registers after the
group-action map. This converts phase information into computational basis states for
measurement.

### Gate Decomposition (per register)

The standard n-qubit QFT uses:
- **n Hadamard gates**: create superposition
- **n(n-1)/2 controlled phase rotations**: CR_k with angle 2π/2^k for k=2..n
- **⌊n/2⌋ SWAP gates**: bit reversal

The inverse QFT reverses the gate order and conjugates all phases (sign → -sign
on controlled rotations). Hadamard and SWAP are self-adjoint.

### Gate Counts (Oath-64, dual register)

| Gate Type | Per Register | Total (×2) |
|-----------|-------------|------------|
| Hadamard | 64 | 128 |
| Controlled-Phase | 2,016 | 4,032 |
| SWAP | 32 | 64 |
| **Total QFT** | **2,112** | **4,224** |

This is <0.1% of the EC arithmetic cost (~34M Toffoli for Oath-64).

### Extended Gate Set

QFT gates are represented by the `QuantumGate` enum (in `quantum_gate.rs`), which
wraps the existing reversible `Gate` (NOT/CNOT/Toffoli) plus:
- `Hadamard { target }` — creates/destroys superposition
- `ControlledPhase { control, target, k, sign }` — phase rotation 2π/2^k
- `Swap { qubit_a, qubit_b }` — QFT bit reversal
- `Measure { qubit, classical_bit }` — computational basis measurement

All gates export to valid OpenQASM 3.0 via `to_qasm()`.

### Classical Verification

The QFT implementation is verified by gate-by-gate state-vector simulation on small
registers (3-4 qubits), confirming exact agreement with the O(N^2) direct DFT matrix.
This verifies the gate decomposition, qubit ordering convention (LSB-first), and
phase signs for both forward and inverse QFT.

## Measurement + Classical Recovery

After inverse QFT, both exponent registers are measured in the computational basis,
yielding a pair (c, d) satisfying:

```
c + k·d ≡ 0 (mod r)
```

where k is the secret discrete log and r is the group order.

### Recovery Methods

1. **Direct modular inversion**: k = -c · d⁻¹ (mod r). Requires gcd(d, r) = 1.
   For prime r, this succeeds for all d ≠ 0.

2. **Multi-measurement recovery**: Combines multiple (c, d) pairs via pairwise
   differences to handle cases where individual d values share factors with r.

3. **Continued fraction expansion**: Extracts rational approximations from
   measurement outcomes when the group order must be inferred (general Shor).

The end-to-end `ShorsEcdlp` pipeline (in `shor.rs`) composes all stages and
verifies recovery by checking [k]G = Q.
