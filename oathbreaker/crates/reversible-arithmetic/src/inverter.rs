use crate::gates::Gate;
use crate::resource_counter::ResourceCounter;

/// Reversible modular inversion via Fermat's little theorem.
///
/// Computes a^(p-2) mod p via reversible square-and-multiply.
/// For p = 2^64 - 2^32 + 1:
///   p - 2 = 0xFFFF_FFFE_FFFF_FFFF
///   Hamming weight ≈ 63, so ~63 multiplications + 63 squarings.
///
/// Each intermediate squaring must be uncomputed to free ancilla qubits.
pub struct FermatInverter {
    /// Number of bits in the field.
    pub n: usize,
}

impl FermatInverter {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the gate sequence for modular inversion via Fermat.
    ///
    /// This is the most expensive single operation in the Shor circuit.
    /// Each step requires a reversible multiplication plus uncomputation.
    pub fn forward_gates(
        &self,
        input_offset: usize,
        result_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // Reversible Fermat inversion: compute a^(p-2) mod p.
        //
        // p - 2 = 0xFFFFFFFEFFFFFFFF for Goldilocks (p = 2^64 - 2^32 + 1).
        // Binary representation: bits 0-31 are all 1, bit 32 is 0, bits 33-63 are all 1.
        // Hamming weight = 63, so square-and-multiply needs 63 squarings + 62 multiplies.
        //
        // Each step uses Bennett's compute-copy-uncompute:
        // 1. Multiply: workspace ← current * power (reversible multiply)
        // 2. Copy: CNOT result to accumulator register
        // 3. Uncompute: reverse the multiply gates to clean workspace
        //
        // We use a sequence of workspace registers for each multiply stage.

        let n = self.n;
        let mut gates = Vec::new();

        // The exponent p-2 in binary (little-endian)
        let p_minus_2: u64 = 0xFFFF_FFFE_FFFF_FFFF;

        // Workspace layout:
        // workspace[0..n]:       acc   — running accumulator (current result)
        // workspace[n..2n]:      sq    — squaring scratch (also used as mul result temp)
        // workspace[2n..4n+1]:   mul_w — multiplier's internal 2n-bit accumulator + 1 carry bit
        //
        // Total: 4n+1 qubits.  The nested ReversibleMultiplier (invoked by ReversibleSquarer)
        // uses 2n+1 qubits at workspace_offset=mul_w, reaching up to mul_w + 2n + 1 =
        // workspace_offset + 4n + 1.  We allocate this full range to avoid overlap.
        let acc_reg = workspace_offset;
        let sq_work = workspace_offset + n;
        let mul_work = workspace_offset + 2 * n;

        counter.allocate_ancilla(4 * n + 1);

        // Initialize accumulator to 1 (the identity for multiplication).
        // In the reversible model, we set bit 0 of the accumulator.
        let g = Gate::Not { target: acc_reg };
        counter.record_gate(&g);
        gates.push(g);

        // Square-and-multiply: scan exponent bits from MSB to LSB.
        // For each bit position (63 down to 0):
        //   1. Square the accumulator
        //   2. If bit is set, multiply by the input
        for bit_pos in (0..64).rev() {
            let bit_set = (p_minus_2 >> bit_pos) & 1 == 1;

            // Step 1: Square the accumulator.
            // Reversible squaring: sq_work ← acc * acc
            // Then copy sq_work → acc, uncompute sq_work.
            let squarer = crate::multiplier::KaratsubaSquarer::new(n);
            let sq_gates = squarer.forward_gates(acc_reg, sq_work, mul_work, counter);
            gates.extend(sq_gates);

            // Swap acc and sq_work: acc now has the squared value
            for i in 0..n {
                // CNOT swap pattern: a^=b, b^=a, a^=b
                let g1 = Gate::Cnot {
                    control: sq_work + i,
                    target: acc_reg + i,
                };
                let g2 = Gate::Cnot {
                    control: acc_reg + i,
                    target: sq_work + i,
                };
                let g3 = Gate::Cnot {
                    control: sq_work + i,
                    target: acc_reg + i,
                };
                counter.record_gate(&g1);
                counter.record_gate(&g2);
                counter.record_gate(&g3);
                gates.push(g1);
                gates.push(g2);
                gates.push(g3);
            }

            // Clear sq_work by running squarer in reverse
            // (it now holds the old acc value which we don't need)
            // Since we swapped, sq_work has old acc^2 XOR'd state.
            // For cleanliness, we zero it via CNOT from the known state.
            for i in 0..n {
                let g = Gate::Cnot {
                    control: acc_reg + i,
                    target: sq_work + i,
                };
                counter.record_gate(&g);
                gates.push(g);
            }

            // Step 2: If exponent bit is set, multiply acc by input.
            if bit_set {
                let mul = crate::multiplier::KaratsubaMultiplier::new(n);
                let mul_gates = mul.forward_gates(
                    acc_reg,
                    input_offset,
                    sq_work, // use sq_work as temp result
                    mul_work,
                    counter,
                );
                gates.extend(mul_gates);

                // Swap result into acc
                for i in 0..n {
                    let g1 = Gate::Cnot {
                        control: sq_work + i,
                        target: acc_reg + i,
                    };
                    let g2 = Gate::Cnot {
                        control: acc_reg + i,
                        target: sq_work + i,
                    };
                    let g3 = Gate::Cnot {
                        control: sq_work + i,
                        target: acc_reg + i,
                    };
                    counter.record_gate(&g1);
                    counter.record_gate(&g2);
                    counter.record_gate(&g3);
                    gates.push(g1);
                    gates.push(g2);
                    gates.push(g3);
                }

                // Clear sq_work
                for i in 0..n {
                    let g = Gate::Cnot {
                        control: acc_reg + i,
                        target: sq_work + i,
                    };
                    counter.record_gate(&g);
                    gates.push(g);
                }
            }
        }

        // Copy final result from accumulator to result register
        for i in 0..n {
            let g = Gate::Cnot {
                control: acc_reg + i,
                target: result_offset + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // NOTE: The accumulator (acc_reg) still holds a^(p-2) after the copy.
        // Full uncomputation of the exponentiation chain (Bennett compute-copy-uncompute)
        // is required to return the workspace to |0⟩ and enable composition.
        // For now we explicitly do NOT call free_ancilla because the 4n+1 workspace
        // qubits are left in a dirty (non-zero) state.  Callers that require clean
        // ancillae must wrap this subroutine in a Bennett uncomputation pass.
        //
        // counter.free_ancilla(4 * n + 1);  // ← omitted: workspace is dirty

        gates
    }
}

/// Reversible modular inversion via binary GCD (Kaliski method).
///
/// Lower gate count than Fermat but more complex control flow.
/// Requires many conditional swaps and shifts.
///
/// Reference: Roetteler et al., "Quantum Resource Estimates for Computing
/// Elliptic Curve Discrete Logarithms" (2017)
pub struct BinaryGcdInverter {
    pub n: usize,
}

impl BinaryGcdInverter {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    pub fn forward_gates(
        &self,
        _input_offset: usize,
        _result_offset: usize,
        _workspace_offset: usize,
        _counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // NOTE: The full reversible binary GCD (Kaliski/Roetteler) inverter requires
        // a complete implementation of the reversible extended-binary-GCD update rules
        // for (u, v, r, s) with proper conditional swaps and shifts at each of the 2n
        // iterations.  The previous stub produced silently incorrect output because
        // r_off was initialised to 0 and never updated with the computed inverse.
        //
        // Use FermatInverter for a working inversion circuit.  This placeholder is
        // retained so the type exists for future implementation.
        todo!("BinaryGcdInverter is not yet implemented; use FermatInverter instead")
    }
}
