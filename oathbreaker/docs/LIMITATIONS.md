# Limitations and Scope

## What This Project Proves

- **Classical arithmetic correctness**: The reversible circuit produces outputs matching the classical EC reference implementation on all tested basis-state inputs.
- **Reversible consistency**: Every gate is self-inverse; the circuit can be run forward and backward.
- **Ancilla cleanup**: All ancilla qubits return to |0> after uncomputation on all tested inputs.
- **Resource count accuracy**: The circuit uses exactly the reported number of qubits and gates.
- **Execution trace correctness**: The SP1 Groth16 proof attests that the above properties hold for N random inputs.
- **Cost attribution accuracy**: Per-subsystem Toffoli costs (doublings, additions, inversion, QROM, affine recovery) are measured via ResourceCounter snapshots, not estimated.

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
Scaling projections from 32-bit to 256-bit are based on asymptotic scaling laws. The benchmark reports three projection models:
- **Karatsuba O(n^2.585)**: Primary model reflecting the implemented Karatsuba multiplier — projects ~1.2B Toffoli at 256-bit
- **Empirical fit**: Least-squares fit from measured 16-bit and 32-bit tiers (exponent ~2.46)
- **Schoolbook O(n^3)**: Legacy model for comparison — projects ~3.4B Toffoli at 256-bit

Actual 256-bit circuits would differ due to:
- Constant factor improvements in industrial implementations
- Hardware-specific compilation optimizations
- Measurement-based uncomputation (not implemented; requires mid-circuit measurement)
- Multi-limb arithmetic overhead (256-bit requires 4× 64-bit limbs)

### No measurement-based uncomputation
The circuit uses Bennett's compute-copy-uncompute pattern exclusively, resulting in ~2x gate overhead compared to measurement-based approaches (e.g., Litinski 2023). Measurement-based uncomputation requires mid-circuit measurement and classical feedforward, which is hardware-dependent and not universally available.

### Intermediate uncomputation is incomplete
The Jacobian mixed addition and doubling circuits leave some intermediate registers dirty (Z1^2, Z1^3, U2, S2, H^2, H^3, X1*H^2 in mixed-add; analogous intermediates in doubling). Full uncomputation would approximately double the gate count. This is a known limitation documented in the source code.

### Comparison to Google is approximate
Google's March 2026 paper discloses resource estimates but not the circuit itself. Our comparison is based on published numbers and scaling projections. Google's implementation uses unknown optimizations that may significantly reduce gate counts.

## Verification Scope Summary

| Property                         | Status   |
|----------------------------------|----------|
| Classical arithmetic correctness | PROVEN   |
| Reversible consistency           | PROVEN   |
| Ancilla cleanup (return to 0)   | PROVEN   |
| Resource count accuracy          | PROVEN   |
| Cost attribution accuracy        | PROVEN   |
| Quantum superposition behavior   | ASSUMED  |
| Measurement-based uncomputation  | NOT IMPL |
| QFT + measurement recovery      | DEFERRED |
| Hardware execution success       | UNKNOWN  |
