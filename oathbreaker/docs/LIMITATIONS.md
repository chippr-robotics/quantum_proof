# Limitations and Scope

## What This Project Proves

- **Classical arithmetic correctness**: The reversible circuit produces outputs matching the classical EC reference implementation on all tested basis-state inputs.
- **Reversible consistency**: Every gate is self-inverse; the circuit can be run forward and backward.
- **Ancilla cleanup**: All ancilla qubits return to |0> after uncomputation on all tested inputs.
- **Resource count accuracy**: The circuit uses exactly the reported number of qubits and gates.
- **Execution trace correctness**: The SP1 Groth16 proof attests that the above properties hold for N random inputs.

## What This Project Does NOT Prove

### No quantum execution or amplitude simulation
The circuit is executed classically on individual basis states. We do not simulate quantum superposition, entanglement, or amplitude interference. Correctness over superpositions is **assumed** via the reversibility property: a reversible circuit that produces correct outputs on all classical basis inputs will produce correct outputs on arbitrary superpositions thereof. This is a mathematical property of unitary operations, not something we demonstrate empirically.

### No full Shor algorithm in v1
The project implements the **coherent group-action map** [a]G + [b]Q — the computationally dominant component (>99% of qubits and gates). The remaining components for full Shor completion are:
- Dual-register Quantum Fourier Transform (O(n^2) gates, <0.1% of cost)
- Quantum measurement + classical reconstruction loop
- Continued fraction / lattice recovery of the discrete log

These are well-understood published constructions and are deferred to v2.

### No quantum hardware execution
No quantum computer currently exists that can execute this circuit. The circuit description is a specification for future hardware, not a claim of current executability.

### Resource projections are estimates
Scaling projections from 32-bit to 256-bit are based on asymptotic scaling laws (O(n) qubits, O(n^2.585) Toffoli with Karatsuba). The benchmark reports three projection models:
- **Karatsuba O(n^2.585)**: Primary model reflecting the implemented Karatsuba multiplier
- **Schoolbook O(n^3)**: Legacy model for comparison
- **Empirical fit**: Least-squares fit from measured 16-bit and 32-bit tiers

Actual 256-bit circuits would differ due to:
- Constant factor improvements in industrial implementations
- Hardware-specific compilation optimizations
- Measurement-based uncomputation (not implemented; requires mid-circuit measurement)

### Fermat inversion is suboptimal
The v1 implementation uses Fermat's little theorem for the single final inversion
(Jacobian Z → affine conversion), contributing O(n^2.585) gates (using Karatsuba multiplier). Known optimizations include:
- **Projective coordinates**: Implemented in v1 — Jacobian projective coordinates eliminate all per-addition inversions, reducing the total from ~128 inversions (affine) to exactly 1 (final affine conversion)
- **Binary GCD / Kaliski inversion**: O(n^2) reversible gates — implemented as an alternative inverter, though Fermat remains the default
- **Karatsuba multiplication**: Implemented — reduces per-multiply cost from O(n^2) to O(n^1.585), changing overall scaling from O(n^3) to O(n^2.585)

### Comparison to Google is approximate
Google's March 2026 paper discloses resource estimates but not the circuit itself. Our comparison is based on published numbers and scaling projections. Google's implementation uses unknown optimizations that may significantly reduce gate counts.

## Verification Scope Summary

| Property                         | Status   |
|----------------------------------|----------|
| Classical arithmetic correctness | PROVEN   |
| Reversible consistency           | PROVEN   |
| Ancilla cleanup (return to 0)   | PROVEN   |
| Resource count accuracy          | PROVEN   |
| Quantum superposition behavior   | ASSUMED  |
| QFT + measurement recovery      | DEFERRED |
| Hardware execution success       | UNKNOWN  |
