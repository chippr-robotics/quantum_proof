# Circuit Architecture

## Overview

The Oathbreaker circuit implements the **coherent double-scalar group-action map**:

```
|a⟩|b⟩|O⟩ → |a⟩|b⟩|[a]G + [b]Q⟩
```

This is the computationally dominant component (>99% of qubits and gates) of Shor's
ECDLP algorithm. Every classical computation is decomposed into NOT, CNOT, and
Toffoli gates operating on a qubit register model.

The circuit targets the **Oath-64** curve over the Goldilocks field GF(2^64 - 2^32 + 1).

## Register Layout (Jacobian Projective)

```
┌──────────────────────┐
│ Exponent A Register  │  64 qubits  (scalar for [a]G)
│ [a_0, a_1, ..., a_63]│
├──────────────────────┤
│ Exponent B Register  │  64 qubits  (scalar for [b]Q)
│ [b_0, b_1, ..., b_63]│
├──────────────────────┤
│ Point X Register     │  64 qubits  (Jacobian X coordinate)
│ [X_0, X_1, ..., X_63]│
├──────────────────────┤
│ Point Y Register     │  64 qubits  (Jacobian Y coordinate)
│ [Y_0, Y_1, ..., Y_63]│
├──────────────────────┤
│ Point Z Register     │  64 qubits  (Jacobian Z coordinate)
│ [Z_0, Z_1, ..., Z_63]│
├──────────────────────┤
│ Ancilla Pool         │  ~N qubits  (dynamically allocated)
│ [anc_0, ..., anc_N]  │
└──────────────────────┘
```

**Note**: Jacobian adds one n-bit Z register (+64 qubits) vs affine.
The tradeoff: +20% qubits, −6× gate count (elimination of per-op inversions).

**Primary qubits**: 5n = 320 (two exponent registers + Jacobian point (X,Y,Z))
**Ancilla qubits**: Dynamic, depends on uncomputation strategy (~300-500)

## Group-Action Circuit Flow

```
1. Controlled [a]G computation:
   For each window of exponent register A:
     - w doublings of accumulator
     - Table lookup from precomputed [0]G..[2^w-1]G
     - Conditional point addition to accumulator
     - Uncompute lookup ancillae

2. Controlled [b]Q addition:
   For each window of exponent register B:
     - Table lookup from precomputed [0]Q..[2^w-1]Q
     - Conditional point addition to accumulator (adds to [a]G result)
     - Uncompute lookup ancillae

3. QFT on both exponent registers (v2 — estimated, not executed in v1)
```

## Gate Decomposition: Point Addition

### Jacobian Mixed Addition (Primary — v1)

The accumulator is in Jacobian projective coordinates (X, Y, Z). Table entries
are in affine (x, y). Mixed addition avoids **all per-addition inversions**:

1. **Squaring** (Z₁²): O(n²) Toffoli
2. **Multiplication** (Z₁³ = Z₁² · Z₁): O(n²) Toffoli
3. **Multiplication** (U₂ = x₂ · Z₁²): O(n²) Toffoli
4. **Multiplication** (S₂ = y₂ · Z₁³): O(n²) Toffoli
5. **Subtraction** (H = U₂ - X₁): O(n) Toffoli
6. **Subtraction** (R = S₂ - Y₁): O(n) Toffoli
7. **Squaring** (H²): O(n²) Toffoli
8. **Multiplication** (H³ = H² · H): O(n²) Toffoli
9. **Multiplication** (X₁·H²): O(n²) Toffoli
10. **Squaring** (R²): O(n²) Toffoli
11. **Subtractions** (X₃ = R² - H³ - 2·X₁·H²): O(n) Toffoli
12. **Multiplication + subtraction** (Y₃): O(n²) Toffoli
13. **Multiplication** (Z₃ = Z₁ · H): O(n²) Toffoli
14. **Uncomputation**: Reversal of intermediates

**Total: ~16 field multiplications, 0 inversions.**

A single Fermat inversion at the end converts Z → affine. One inversion
for the entire scalar multiplication instead of one per point addition.

### Affine Addition (Reference Implementation)

The affine version (retained for comparison) decomposes into 6 multiplications +
1 inversion per addition. The inversion is 94% of the gate cost.

```
┌─────────────────────┬──────────────┬──────────────┐
│ Metric              │ Affine+Fermat│ Jacobian+1inv│
├─────────────────────┼──────────────┼──────────────┤
│ Mul-equivalents/add │   ~102       │   ~16        │
│ Inversions (total)  │   ~128       │   1          │
│ Gate ratio          │   1.0×       │   ~0.16×     │
└─────────────────────┴──────────────┴──────────────┘
```

## Ancilla Strategies

### Eager Uncomputation
- Free ancillae immediately after use
- Minimizes qubit count
- Increases gate count (roughly 2x for uncomputation)

### Deferred Uncomputation (Bennett's Pebble Game)
- Keep intermediates alive
- Uncompute in bulk at the end
- Fewer gates but more qubits

Both strategies are implemented. The qubit/gate tradeoff curve is a primary
research output.

## Windowed Scalar Multiplication

Applied independently to both scalar registers:

| Window Size | Iterations | Table Size | Tradeoff |
|-------------|-----------|------------|----------|
| 1 | 64 | 2 points | Minimal memory |
| 4 | 16 | 16 points | Balanced |
| 8 | 8 | 256 points | Fewer iterations |
| 16 | 4 | 65536 points | Fewest iterations |

## QFT Component (v2)

QFT applied to each 64-qubit exponent register:
- 128 Hadamard gates (64 per register)
- 4,032 controlled phase rotations
- 64 SWAP gates
- Total: ~4,224 gates (<0.1% of EC arithmetic cost)

Described and resource-counted in v1. Execution deferred to v2.
