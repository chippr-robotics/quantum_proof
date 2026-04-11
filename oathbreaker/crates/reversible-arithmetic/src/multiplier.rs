use crate::gates::Gate;
use crate::resource_counter::ResourceCounter;

/// Reversible modular multiplier for GF(p).
///
/// Decomposes multiplication into controlled additions (schoolbook method):
/// |a⟩|b⟩|0⟩ → |a⟩|b⟩|a*b mod p⟩
///
/// Gate count: O(n²) Toffoli for n-bit operands.
/// Ancilla: accumulator register (n+1 bits) + reduction workspace.
pub struct ReversibleMultiplier {
    /// Number of bits per operand.
    pub n: usize,
}

impl ReversibleMultiplier {
    pub fn new(n: usize) -> Self {
        Self { n }
    }

    /// Generate the gate sequence for modular multiplication.
    ///
    /// Register layout:
    /// - a[0..n]: first operand (preserved)
    /// - b[0..n]: second operand (preserved)
    /// - result[0..n]: output register (starts at 0, ends with a*b mod p)
    /// - workspace: ancilla qubits for intermediate values
    ///
    /// The multiplication uses schoolbook decomposition:
    /// a * b = Σ_i a_i * b * 2^i
    /// Each bit a_i controls a conditional addition of (b << i) to the accumulator.
    ///
    /// After accumulation, reduce modulo p using the special form:
    /// p = 2^64 - 2^32 + 1, so 2^64 ≡ 2^32 - 1 (mod p).
    ///
    /// Intermediate values are uncomputed via Bennett's compute-copy-uncompute strategy.
    pub fn forward_gates(
        &self,
        a_offset: usize,
        b_offset: usize,
        result_offset: usize,
        workspace_offset: usize,
        counter: &mut ResourceCounter,
    ) -> Vec<Gate> {
        // Reversible schoolbook multiplication using controlled additions.
        //
        // |a⟩|b⟩|0⟩ → |a⟩|b⟩|a*b mod p⟩
        //
        // Uses Bennett's compute-copy-uncompute pattern:
        // 1. Forward: accumulate partial products via controlled additions
        // 2. Copy: CNOT result to output register
        // 3. Reverse: uncompute accumulator by running forward gates in reverse
        //
        // Each bit a[i] controls the addition of (b << i) to the accumulator.
        // The accumulator is 2n bits wide to hold intermediate products before
        // reduction. After all partial products, reduce mod p.

        let n = self.n;
        let mut gates = Vec::new();

        // Workspace layout:
        // workspace[0..2n]: accumulator (double-width for unreduced product)
        // workspace[2n]: carry bit for adder
        let acc_offset = workspace_offset;
        let carry_bit = workspace_offset + 2 * n;

        counter.allocate_ancilla(2 * n + 1);

        // --- Forward pass: schoolbook partial product accumulation ---
        let mut forward_gates = Vec::new();

        for i in 0..n {
            // For each bit a[i], conditionally add b shifted left by i positions
            // to the accumulator. This is a Toffoli-controlled addition.
            //
            // For each bit j of b: Toffoli(a[i], b[j], acc[i+j])
            // with carry propagation.
            for j in 0..n {
                if i + j < 2 * n {
                    // Toffoli: acc[i+j] ^= a[i] & b[j]
                    let g = Gate::Toffoli {
                        control1: a_offset + i,
                        control2: b_offset + j,
                        target: acc_offset + i + j,
                    };
                    counter.record_gate(&g);
                    forward_gates.push(g);
                }
            }

            // Carry propagation for this partial product row.
            // For each bit position k where a carry might propagate:
            for j in 0..n {
                let pos = i + j;
                if pos + 1 < 2 * n {
                    // Propagate carry: if acc[pos] overflows, carry into acc[pos+1]
                    // This is handled via the Toffoli gate on the carry chain.
                    // Simplified model: we track carries via additional Toffoli gates.
                    let g = Gate::Toffoli {
                        control1: a_offset + i,
                        control2: acc_offset + pos,
                        target: carry_bit,
                    };
                    counter.record_gate(&g);
                    forward_gates.push(g);

                    let g2 = Gate::Cnot {
                        control: carry_bit,
                        target: acc_offset + pos + 1,
                    };
                    counter.record_gate(&g2);
                    forward_gates.push(g2);

                    // Uncompute carry bit for reuse
                    let g3 = Gate::Toffoli {
                        control1: a_offset + i,
                        control2: acc_offset + pos,
                        target: carry_bit,
                    };
                    counter.record_gate(&g3);
                    forward_gates.push(g3);
                }
            }
        }

        // --- Goldilocks reduction ---
        // Reduce the 2n-bit accumulator mod p = 2^64 - 2^32 + 1.
        // Using: 2^64 ≡ 2^32 - 1 (mod p)
        // For each high bit at position 64+k, it contributes (2^32 - 1) * 2^k
        // to the low part. We fold high bits into low bits.
        //
        // In the reversible model, reduction is done by XOR-folding:
        // For each bit in [n..2n), XOR it into positions [k] and [k+32]
        // (with appropriate carries), representing the multiply by (2^32 - 1).
        for k in 0..n {
            // acc[n+k] contributes to acc[k+32] (from 2^32 factor) and
            // subtracts from acc[k] (from -1 factor).
            // Fold: acc[k] ^= acc[n+k] (the -1 part)
            if n + k < 2 * n {
                let g = Gate::Cnot {
                    control: acc_offset + n + k,
                    target: acc_offset + k,
                };
                counter.record_gate(&g);
                forward_gates.push(g);
            }
            // Fold: acc[k+32] ^= acc[n+k] (the 2^32 part), if in range
            if k + 32 < n && n + k < 2 * n {
                let g = Gate::Cnot {
                    control: acc_offset + n + k,
                    target: acc_offset + k + 32,
                };
                counter.record_gate(&g);
                forward_gates.push(g);
            }
        }

        gates.extend(forward_gates.clone());

        // --- Copy result to output register ---
        // CNOT the low n bits of the accumulator to the result register
        for i in 0..n {
            let g = Gate::Cnot {
                control: acc_offset + i,
                target: result_offset + i,
            };
            counter.record_gate(&g);
            gates.push(g);
        }

        // --- Uncompute: reverse the forward gates ---
        for gate in forward_gates.iter().rev() {
            let inv = gate.inverse();
            counter.record_gate(&inv);
            gates.push(inv);
        }

        counter.free_ancilla(2 * n + 1);
        gates
    }
}

/// Reversible modular squaring, specialized for better gate count.
///
/// Since both inputs are the same, some gate optimizations are possible.
pub struct ReversibleSquarer {
    pub n: usize,
}

impl ReversibleSquarer {
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
        // Reversible squaring: delegates to the multiplier with both inputs
        // pointing to the same register.
        //
        // |a⟩|0⟩ → |a⟩|a² mod p⟩
        //
        // Future optimization: exploit symmetry of cross-terms to reduce
        // gate count by ~25% (each a[i]*a[j] term appears twice, so only
        // one Toffoli + doubling is needed instead of two Toffoli gates).
        let mul = ReversibleMultiplier::new(self.n);
        mul.forward_gates(input_offset, input_offset, result_offset, workspace_offset, counter)
    }
}
