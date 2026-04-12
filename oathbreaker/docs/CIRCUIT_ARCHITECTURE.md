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

**Measured qubit counts** (including all ancillae):

| Tier | Primary (5n) | One-Hot (2^w) | Ancillae | Total |
|------|-------------|--------------|----------|-------|
| Oath-8 | 40 | 16 | ~239 | 295 |
| Oath-16 | 80 | 16 | ~759 | 855 |
| Oath-32 | 160 | 256 | ~2,432 | 2,848 |

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

5. QFT on both exponent registers (v2 — estimated, not executed in v1)
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

### Jacobian Point Doubling (6S + 3M = 9 field operations)

Uses proper Cuccaro arithmetic for all constant multiplications (×2, ×3, ×4, ×8):

1. **Squaring** (X₁²): Karatsuba squarer
2. **Squaring** (Y₁²) → A: Karatsuba squarer
3. **Multiplication** (X₁ · A): Karatsuba multiplier
4. **Constant multiply** (4·(X₁·A)) → B: 4 Cuccaro additions
5. **Squaring** (A²): Karatsuba squarer
6. **Constant multiply** (8·A²) → C: 8 Cuccaro additions
7. **Constant multiply** (3·X₁²) → D: 3 Cuccaro additions
8. **Squaring** (D²): Karatsuba squarer
9. **Subtraction** (X₃ = D² - 2·B): Cuccaro subtract
10. **Subtraction + multiplication** (Y₃ = D·(B - X₃) - C)
11. **Multiplication + addition** (Z₃ = 2·Y₁·Z₁): Cuccaro add

**Workspace**: 14n + 2 qubits (including dedicated a_sq register and sub_carry bit).

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
- Hardware-dependent; deferred to v2

## Windowed Scalar Multiplication

Applied independently to both scalar registers:

| Window Size | Iterations (n=32) | Table Size | Toffoli (Oath-32) | Qubits |
|-------------|-------------------|------------|-------------------|--------|
| 1 | 32 | 2 points | 13.4M | 2,340 |
| 2 | 16 | 4 points | 9.5M | 2,344 |
| 4 | 8 | 16 points | 7.6M | 2,368 |
| **8** | **4** | **256 points** | **5.76M** | **2,848** |

w=8 is monotonically best for Toffoli across all measured tiers.

## QFT Component (v2)

QFT applied to each n-qubit exponent register (n=64 for Oath-64):
- 128 Hadamard gates (64 per register)
- 4,032 controlled phase rotations
- 64 SWAP gates
- Total: ~4,224 gates (<0.1% of EC arithmetic cost)

Described and resource-counted in v1. Execution deferred to v2.
