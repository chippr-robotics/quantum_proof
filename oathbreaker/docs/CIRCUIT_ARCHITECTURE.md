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

## Register Layout

```
┌──────────────────────┐
│ Exponent A Register  │  64 qubits  (scalar for [a]G)
│ [a_0, a_1, ..., a_63]│
├──────────────────────┤
│ Exponent B Register  │  64 qubits  (scalar for [b]Q)
│ [b_0, b_1, ..., b_63]│
├──────────────────────┤
│ Point X Register     │  64 qubits  (accumulator x-coordinate)
│ [x_0, x_1, ..., x_63]│
├──────────────────────┤
│ Point Y Register     │  64 qubits  (accumulator y-coordinate)
│ [y_0, y_1, ..., y_63]│
├──────────────────────┤
│ Ancilla Pool         │  ~N qubits  (dynamically allocated)
│ [anc_0, ..., anc_N]  │
└──────────────────────┘
```

**Primary qubits**: 4n = 256 (two exponent registers + point accumulator)
**Ancilla qubits**: Dynamic, depends on uncomputation strategy

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

## Gate Decomposition: Point Addition (Affine)

The EC point addition P + Q = R is decomposed into:

1. **Subtraction** (Δx = x₂ - x₁): O(n) Toffoli
2. **Subtraction** (Δy = y₂ - y₁): O(n) Toffoli
3. **Inversion** (Δx⁻¹): O(n³) Toffoli (Fermat) or O(n²) (binary GCD)
4. **Multiplication** (λ = Δy · Δx⁻¹): O(n²) Toffoli
5. **Squaring** (λ²): O(n²) Toffoli
6. **Subtraction** (x₃ = λ² - x₁ - x₂): O(n) Toffoli
7. **Multiplication + subtraction** (y₃): O(n²) Toffoli
8. **Uncomputation**: Reversal of steps 1-5 (doubles gate count for intermediates)

**Design note**: v1 uses affine coordinates + Fermat inversion. Future optimization:
projective coordinates to eliminate per-addition inversion entirely.

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
