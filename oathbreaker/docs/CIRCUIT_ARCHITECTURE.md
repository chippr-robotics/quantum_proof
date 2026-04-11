# Circuit Architecture

## Overview

The Oathbreaker circuit implements Shor's algorithm for ECDLP using reversible (quantum-compatible) gates. Every classical computation is decomposed into NOT, CNOT, and Toffoli gates operating on a qubit register model.

The circuit targets the **Oath-64** curve — an elliptic curve over the Goldilocks field GF(2^64 - 2^32 + 1).

## Register Layout

```
┌──────────────────────┐
│ Scalar Register      │  64 qubits  (phase estimation input)
│ [s_0, s_1, ..., s_63]│
├──────────────────────┤
│ Point X Register     │  64 qubits  (EC point x-coordinate)
│ [x_0, x_1, ..., x_63]│
├──────────────────────┤
│ Point Y Register     │  64 qubits  (EC point y-coordinate)
│ [y_0, y_1, ..., y_63]│
├──────────────────────┤
│ Ancilla Pool         │  ~N qubits  (dynamically allocated)
│ [a_0, a_1, ..., a_N] │
└──────────────────────┘
```

## Gate Decomposition: Point Addition

The EC point addition P + Q = R is decomposed into:

1. **Subtraction** (Δx = x₂ - x₁): O(n) Toffoli
2. **Subtraction** (Δy = y₂ - y₁): O(n) Toffoli
3. **Inversion** (Δx⁻¹): O(n³) Toffoli (Fermat) or O(n²) (binary GCD)
4. **Multiplication** (λ = Δy · Δx⁻¹): O(n²) Toffoli
5. **Squaring** (λ²): O(n²) Toffoli
6. **Subtraction** (x₃ = λ² - x₁ - x₂): O(n) Toffoli
7. **Multiplication + subtraction** (y₃): O(n²) Toffoli
8. **Uncomputation**: Reversal of steps 1-5 (doubles gate count for intermediates)

## Ancilla Strategies

### Eager Uncomputation
- Free ancillae immediately after use
- Minimizes qubit count
- Increases gate count (roughly 2× for uncomputation)

### Deferred Uncomputation (Bennett's Pebble Game)
- Keep intermediates alive
- Uncompute in bulk at the end
- Fewer gates but more qubits

## Windowed Scalar Multiplication

Instead of processing scalar bits one at a time (64 iterations), we use windowed arithmetic:

- **Window size w**: Process w bits per iteration (64/w iterations)
- **Precomputed table**: [0]G through [2^w - 1]G stored as circuit constants
- **Per window**: w doublings + 1 table lookup + 1 conditional addition

| Window Size | Iterations | Table Size | Tradeoff |
|-------------|-----------|------------|----------|
| 1 | 64 | 2 points | Minimal memory, most iterations |
| 4 | 16 | 16 points | Balanced |
| 8 | 8 | 256 points | Fewer iterations, larger table |
| 16 | 4 | 65536 points | Fewest iterations, largest table |

## QFT Component

The Quantum Fourier Transform is applied to the scalar register after the controlled scalar multiplication. For 64 qubits:

- 64 Hadamard gates
- 2,016 controlled phase rotations
- 32 SWAP gates
- Trivial resource cost compared to the EC arithmetic
