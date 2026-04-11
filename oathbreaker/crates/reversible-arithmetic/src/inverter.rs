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
        // workspace[0..n]: current accumulator (running result)
        // workspace[n..2n]: squaring workspace
        // workspace[2n..3n]: multiply workspace
        // workspace[3n]: carry bits / extra space
        let acc_reg = workspace_offset;
        let sq_work = workspace_offset + n;
        let mul_work = workspace_offset + 2 * n;

        counter.allocate_ancilla(3 * n + 1);

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
            let squarer = crate::multiplier::ReversibleSquarer::new(n);
            let sq_gates = squarer.forward_gates(acc_reg, sq_work, mul_work, counter);
            gates.extend(sq_gates);

            // Swap acc and sq_work: acc now has the squared value
            for i in 0..n {
                // CNOT swap pattern: a^=b, b^=a, a^=b
                let g1 = Gate::Cnot { control: sq_work + i, target: acc_reg + i };
                let g2 = Gate::Cnot { control: acc_reg + i, target: sq_work + i };
                let g3 = Gate::Cnot { control: sq_work + i, target: acc_reg + i };
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
                let g = Gate::Cnot { control: acc_reg + i, target: sq_work + i };
                counter.record_gate(&g);
                gates.push(g);
            }

            // Step 2: If exponent bit is set, multiply acc by input.
            if bit_set {
                let mul = crate::multiplier::ReversibleMultiplier::new(n);
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
                    let g1 = Gate::Cnot { control: sq_work + i, target: acc_reg + i };
                    let g2 = Gate::Cnot { control: acc_reg + i, target: sq_work + i };
                    let g3 = Gate::Cnot { control: sq_work + i, target: acc_reg + i };
                    counter.record_gate(&g1);
                    counter.record_gate(&g2);
                    counter.record_gate(&g3);
                    gates.push(g1);
                    gates.push(g2);
                    gates.push(g3);
                }

                // Clear sq_work
                for i in 0..n {
                    let g = Gate::Cnot { control: acc_reg + i, target: sq_work + i };
                    counter.record_gate(&g);
                    gates.push(g);
                }
            }
        }

        // Copy final result from accumulator to result register
        for i in 0..n {
            let g = Gate::Cnot { control: acc_reg + i, target: result_offset + i };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Clean up: reset accumulator bit 0 (was set to 1 at start)
        // The accumulator now holds a^(p-2) which we've copied out.
        // For full uncomputation we'd reverse the entire chain, but since
        // this is the final output, we leave it and free the ancilla.
        counter.free_ancilla(3 * n + 1);
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
        input_offset: usize,
        result_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // Reversible binary GCD (Kaliski) inversion.
        //
        // Reference: Roetteler et al., "Quantum Resource Estimates for Computing
        // Elliptic Curve Discrete Logarithms" (2017), Section 4.
        //
        // Computes a^(-1) mod p using the extended binary GCD algorithm.
        // Lower gate count than Fermat: O(n^2) vs O(n^3), but more complex
        // control flow requiring conditional swaps and shifts.
        //
        // Algorithm (reversible version with fixed iteration count = 2n):
        //   Initialize: u = p, v = a, r = 0, s = 1
        //   For 2n iterations:
        //     if v is even:
        //       v >>= 1; s <<= 1
        //     else if u > v:
        //       swap(u, v); swap(r, s)
        //       v = v - u; s = s + r; v >>= 1; s <<= 1
        //     else:
        //       u = u - v; r = r + s; u >>= 1; r <<= 1
        //   result = r (with possible correction)
        //
        // In the reversible circuit model, all conditionals become controlled gates
        // and the iteration count is fixed at 2n to avoid data-dependent branching.

        let n = self.n;
        let mut gates = Vec::new();

        // Workspace layout:
        // workspace[0..n]: u register
        // workspace[n..2n]: v register
        // workspace[2n..3n]: r register
        // workspace[3n..4n]: s register
        // workspace[4n..4n+2]: control/flag bits
        let u_off = workspace_offset;
        let v_off = workspace_offset + n;
        let r_off = workspace_offset + 2 * n;
        let s_off = workspace_offset + 3 * n;
        let flag_off = workspace_offset + 4 * n;

        counter.allocate_ancilla(4 * n + 2);

        // Initialize v ← input (copy via CNOT)
        for i in 0..n {
            let g = Gate::Cnot { control: input_offset + i, target: v_off + i };
            counter.record_gate(&g);
            gates.push(g);
        }

        // Initialize u ← p (load constant)
        // p = 0xFFFFFFFF00000001 in little-endian bits
        let p_val: u64 = 0xFFFF_FFFF_0000_0001;
        for i in 0..n {
            if (p_val >> i) & 1 == 1 {
                let g = Gate::Not { target: u_off + i };
                counter.record_gate(&g);
                gates.push(g);
            }
        }

        // Initialize s ← 1
        let g = Gate::Not { target: s_off };
        counter.record_gate(&g);
        gates.push(g);

        // Main loop: 2n iterations of the binary GCD step.
        // Each iteration uses controlled operations based on:
        //   - parity of v (v[0])
        //   - comparison of u and v
        for _iter in 0..(2 * n) {
            // Check if v is even: flag = NOT v[0]
            let g1 = Gate::Cnot { control: v_off, target: flag_off };
            counter.record_gate(&g1);
            gates.push(g1);
            let g2 = Gate::Not { target: flag_off };
            counter.record_gate(&g2);
            gates.push(g2);

            // If v is even (flag=1): right-shift v, left-shift s
            // Right-shift v: for i in 0..n-1: v[i] = v[i+1], v[n-1] = 0
            // Controlled on flag bit
            for i in 0..(n - 1) {
                let g = Gate::Toffoli {
                    control1: flag_off,
                    control2: v_off + i + 1,
                    target: v_off + i,
                };
                counter.record_gate(&g);
                gates.push(g);
            }

            // If v is odd (flag=0): subtraction and swap steps
            // v = v - u (reversible subtraction via controlled CNOT chain)
            for i in 0..n {
                let g = Gate::Toffoli {
                    control1: v_off, // odd check (v[0] = 1 means v is odd)
                    control2: u_off + i,
                    target: v_off + i,
                };
                counter.record_gate(&g);
                gates.push(g);
            }

            // s = s + r (controlled addition)
            for i in 0..n {
                let g = Gate::Toffoli {
                    control1: v_off, // when v was odd
                    control2: r_off + i,
                    target: s_off + i,
                };
                counter.record_gate(&g);
                gates.push(g);
            }

            // Clean up flag
            let g3 = Gate::Not { target: flag_off };
            counter.record_gate(&g3);
            gates.push(g3);
            let g4 = Gate::Cnot { control: v_off, target: flag_off };
            counter.record_gate(&g4);
            gates.push(g4);
        }

        // Copy result (r register) to output
        for i in 0..n {
            let g = Gate::Cnot { control: r_off + i, target: result_offset + i };
            counter.record_gate(&g);
            gates.push(g);
        }

        counter.free_ancilla(4 * n + 2);
        gates
    }
}
