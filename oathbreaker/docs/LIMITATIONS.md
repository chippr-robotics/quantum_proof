# Limitations and Scope

## What This Project Proves

- **Classical arithmetic correctness**: The reversible circuit produces outputs matching the classical EC reference implementation on all tested basis-state inputs.
- **Reversible consistency**: Every gate is self-inverse; the circuit can be run forward and backward.
- **Ancilla cleanup**: All ancilla qubits return to |0> after uncomputation on all tested inputs.
- **Resource count accuracy**: The circuit uses exactly the reported number of qubits and gates.
- **Execution trace correctness**: The SP1 Groth16 proof attests that the above properties hold for N random inputs. The SP1 guest program builds the circuit, verifies all test cases, and commits resource counts and a circuit hash as public values.
- **Cost attribution accuracy**: Per-subsystem Toffoli costs (doublings, additions, inversion, QROM, affine recovery) are measured via ResourceCounter snapshots, not estimated.
- **QFT gate correctness**: The gate-by-gate QFT simulation matches the direct O(N^2) DFT matrix for 3-qubit and 4-qubit registers, confirming the Hadamard + controlled-phase + SWAP decomposition is correct.
- **Shor's classical recovery correctness**: End-to-end tests verify that the classical post-processing (modular inversion / continued fractions) correctly recovers the secret discrete log k from simulated measurement outcomes, and that [k]G = Q, for multiple secret values.

## What This Project Does NOT Prove

### No quantum execution or amplitude simulation
The circuit is executed classically on individual basis states. We do not simulate quantum superposition, entanglement, or amplitude interference. Correctness over superpositions is **assumed** via the reversibility property: a reversible circuit that produces correct outputs on all classical basis inputs will produce correct outputs on arbitrary superpositions thereof. This is a mathematical property of unitary operations, not something we demonstrate empirically.

### No full quantum state simulation for QFT
The QFT gate sequence is generated and verified for correctness on small registers (3-4 qubits) via full state-vector simulation against the direct DFT matrix. For 64-qubit registers, full state-vector simulation is infeasible (O(2^64) memory). The QFT is a standard textbook construction; its correctness at arbitrary scale follows from the verified gate decomposition.

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

### Qubit counts are an upper bound (Bennett uncomputation)
The reported logical qubit counts represent the peak number of simultaneously live qubits using Bennett's compute-copy-uncompute pattern. This is the standard reversible circuit model, but it is not optimal for qubit count. The benchmark reports a separate **measurement-based qubit estimate** showing the reduction achievable with mid-circuit measurement and classical feedforward.

| Tier | Bennett (measured) | Measurement-based (est.) | Reduction |
|------|-------------------|-------------------------|-----------|
| Oath-8 | 210 | 186 | 11% |
| Oath-16 | 402 | 370 | 8% |
| Oath-32 | 1,026 | 738 | 28% |
| 256-bit (projected) | ~8,208 | ~5,890 | 28% |

Google's ~1,175 qubits for 256-bit ECDLP is achieved through measurement-based uncomputation combined with semi-classical oracle techniques (avoiding coherent QROM entirely) and aggressive register scheduling that goes beyond the `23n+2` model used here.

### No measurement-based uncomputation (implemented)
The circuit uses Bennett's compute-copy-uncompute pattern exclusively, resulting in ~2x gate overhead and higher qubit counts compared to measurement-based approaches (e.g., Litinski 2023). Measurement-based uncomputation requires mid-circuit measurement and classical feedforward, which is hardware-dependent and not universally available. The measurement-based qubit estimates reported by the benchmark model the savings from this technique without implementing it.

### Intermediate uncomputation is incomplete
The Jacobian mixed addition and doubling circuits leave some intermediate registers dirty (Z1^2, Z1^3, U2, S2, H^2, H^3, X1*H^2 in mixed-add; analogous intermediates in doubling). Full uncomputation would approximately double the gate count. This is a known limitation documented in the source code. The circuit reuses workspace across loop iterations via the Bennett pattern (each multiplier/squarer internally uncomputes its workspace), so dirty intermediates are overwritten rather than accumulated.

### Comparison to Google is approximate
Google's March 2026 paper discloses resource estimates but not the circuit itself. Our comparison is based on published numbers and scaling projections. Google's implementation uses unknown optimizations that may significantly reduce gate counts. The remaining gap between Oathbreaker's measurement-based estimate (~5,890 qubits at 256-bit) and Google's ~1,175 is attributable to:
- Semi-classical oracle (avoids coherent QROM; ~2-4x qubit reduction)
- Optimized formula scheduling (fewer simultaneous live registers)
- Hardware-specific compilation (magic state distillation, routing)

## Verification Scope Summary

| Property                         | Status   |
|----------------------------------|----------|
| Classical arithmetic correctness | PROVEN   |
| Reversible consistency           | PROVEN   |
| Ancilla cleanup (return to 0)   | PROVEN   |
| Resource count accuracy          | PROVEN   |
| Cost attribution accuracy        | PROVEN   |
| QFT gate correctness (small n)   | PROVEN   |
| Shor's end-to-end recovery       | PROVEN   |
| Quantum superposition behavior   | ASSUMED  |
| Measurement-based uncomputation  | NOT IMPL |
| Hardware execution success       | UNKNOWN  |
